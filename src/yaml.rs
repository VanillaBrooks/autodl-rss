use super::error::Error;
use super::rss;
use super::utils;
use super::qbit_data as qbit;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};

use reqwest;
use serde::{Deserialize, Serialize};
use serde_yaml;

#[derive(Debug, Deserialize)]
pub struct FeedManager {
    feeds: Vec<RssFeed>,
    #[serde(default)]
    next_update: u32,
    #[serde(skip)]
    client: Option<reqwest::Client>,

    // rss hashes that we have looked at
    #[serde(default)]
    previous_hashes: HashSet<u64>,

    // private trackers to keep seeding 
    #[serde(default)]
    trackers_to_keep: Vec<String>,

    // qbit hashes that are good and we dont need to recheck
    #[serde(default)]
    good_qbit_hashes: HashSet<String>,

    // qbit hashes that are bad and already paused
    #[serde(default)]
    paused_qbit_hashes: HashSet<String>
}
impl FeedManager {
    // Fetch yaml of configs to download
    pub fn from_yaml(path: &str) -> Result<FeedManager, Error> {
        let file = File::open(path)?;

        let mut yaml: FeedManager = serde_yaml::from_reader(file)?;
        yaml.client = Some(reqwest::Client::new());

        Ok(yaml)
    }

    // check all rss feeds for updates: update, pull torrents, and download them if possible
    pub fn run_update(&mut self) -> Result<u32, Error> {
        let mut next_update_time = 60* 15;
        let epoch = utils::current_unix_time();

        let mut hashes_to_add = HashSet::new();

        self.feeds
            .iter()
            .filter(|x| {
                let diff = epoch - x.last_announce;
                if epoch - x.last_announce > x.update_interval {

                    if x.update_interval < next_update_time {
                        next_update_time = x.update_interval
                    }

                    true

                } else {
                    if diff < next_update_time {
                        next_update_time = diff
                    }
                    false
                }
            })
            .map(|x| x.fetch_new(&self.client.as_ref().unwrap()))
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            .flatten()
            .for_each(|data| {
                if !self.previous_hashes.contains(&data.item_hash) {
                    self.start_qbit_download(&data);
                    hashes_to_add.insert(data.item_hash);
                }
            });

        hashes_to_add.into_iter().for_each(|hash| {
            self.previous_hashes.insert(hash);
        });

        self.next_update = next_update_time;

        Ok(next_update_time)
    }

    // start qbittorrnet's download of a file
    pub fn start_qbit_download(&self, data: &rss::TorrentData) {

        dbg!{"downloading new file"};
        
        let mut post = HashMap::with_capacity(5);

        let save_folder = data.original_matcher.unwrap().save_folder.clone();

        fs::create_dir_all(&save_folder);
        let x = data.write_metadata();

        post.insert("urls", data.download_link.clone());
        post.insert("savepath", save_folder);
        post.insert("sequentialDownload", "true".to_string());

        // dbg!{&post};

        let ans = self
            .client
            .as_ref()
            .unwrap()
            .post("http://localhost:8080/command/download")
            .form(&post)
            .send();

        match ans {
            Ok(response) => {dbg!{response.status()}; },
            Err(_) => ()
        };

        dbg!{&data.title};

    }

    // Stops torrents that are using banned trackers from seeding
    pub fn clear_public_trackers(&mut self) -> Result<(), Error> {
        let cref = self.client.as_ref().unwrap();

        let ans = cref
            .get("http://localhost:8080/query/torrents?filter=completed")
            .send()?;

        let data = qbit::QbitData::from_reader(ans)?;
        for torrent in &data {
            if !self.good_qbit_hashes.contains(&torrent.hash) && !self.paused_qbit_hashes.contains(&torrent.hash) {
                
                let request = format!{"http://localhost:8080/query/propertiesTrackers/{}", &torrent.hash};
                let trackers = cref.get(&request)
                .send()?;

                let specific_torrent_data = match qbit::TrackerData::from_reader(trackers){
                    Ok(data) => data,
                    Err(_) => continue
                };
            
                // the torrent is in an approved tracker. save the hash so we dont check latter
                if self.keep_seeding_tracker(&specific_torrent_data) {
                    self.good_qbit_hashes.insert(torrent.hash.clone());
                }
                // stop the torrent since its completed
                else{

                    dbg!{"stopping torrent"};

                    let mut map = reqwest::header::HeaderMap::new();
                    map.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Fiddler"));

                    let mut form = HashMap::new();
                    form.insert("hash", torrent.hash.to_string());

                    let command_url = format!{"http://localhost:8080/command/pause?hash={}",torrent.hash};
                    let command_url = "http://localhost:8080/command/pause";
                    let response = cref
                        .post(command_url)
                        .headers(map)
                        .form(&form)
                        .send();

                    self.paused_qbit_hashes.insert(torrent.hash.clone());

                }
            }

        }

        Ok(())
    }

    fn keep_seeding_tracker(&self, t_data: &qbit::TrackerData) -> bool {
        let mut keep = false;
        for i in &self.trackers_to_keep {
            if t_data.url().contains(i) {keep = true}
        }
        return keep
    }
}

#[derive(Debug, Deserialize)]
pub struct RssFeed {
    pub url: String,
    pub update_interval: u32,
    #[serde(default)]
    pub last_announce: u32,
    pub matcher: Vec<TorrentMatch>,
}
impl RssFeed {
    pub fn fetch_new(&self, pool: &reqwest::Client) -> Result<Vec<rss::TorrentData>, Error> {
        let mut response = pool.get(&self.url).send()?;
        let data = rss::xml_to_torrents(response)?;

        // let file = File::open("nyaa_si.xml").expect("sample file not found");
        // let mut data = rss::xml_to_torrents(file)?;

        let mut filter_data = data
            .into_iter()
            .map(|mut x| {
                // make sure that the file matches at least one type condition
                let mut condition = false;

                for mat in self.matcher.iter() {
                    if mat.match_title(&x.title) && mat.match_tags(&x.tags) {
                        dbg!{"found match"};
                        condition = true;
                        x.original_matcher = Some(&mat);
                        break;
                    }
                }

                (condition, x)
            })
            .filter(|(condition, data)| *condition)
            .map(|(_, data)| data)
            .collect::<Vec<_>>();

        Ok(filter_data)
    }
}

type Matcher = Option<Vec<Vec<String>>>;
#[derive(Deserialize, Debug)]
pub struct TorrentMatch {
    pub title_wanted: Matcher,
    pub title_banned: Matcher,

    pub tags_wanted: Matcher,
    pub tags_banned: Matcher,
    pub save_folder: String,
}
impl TorrentMatch {
    fn match_title(&self, title_input: &String) -> bool {
        // dbg!{title_input};
        let mut good_title = true;

        //
        // TODO: make this a better parsing
        //

        if let Some(wanted_titles) = &self.title_wanted {
            for title in wanted_titles {

                if !title_input.contains_(&title){
                    good_title = false;
                    break
                }
            }
        }

        if let Some(banned_title) = &self.title_banned {
            for title in banned_title {
                if title_input.contains_(&title){
                    good_title = false;
                    break
                }
            }
        }

        return good_title;
    }

    // make sure the HashSet is all lowercase
    fn match_tags(&self, tag_input: &HashSet<String>) -> bool {
        let mut good_tags = true;

        // dbg!{&tag_input};

        //
        // TODO: make this a better parsing
        //

        if let Some(tags_wanted) = &self.tags_wanted {
            for tag in tags_wanted {
                if !tag_input.contains_(tag){
                    good_tags = false;
                    break
                }
            }
        }

        if let Some(tags_banned) = &self.tags_banned {
            for tag in tags_banned {
                if tag_input.contains_(tag) {
                    good_tags = false;
                    break;
                }
            }
        }

        return good_tags;
    }
}

trait Contains_ {
    fn contains_(&self, value: &Vec<String>) -> bool;
}


impl Contains_ for HashSet<String> {
    fn contains_(&self, or_tags_group: &Vec<String>) -> bool{
        let mut good = false;

        for tag in or_tags_group {
            if self.contains(tag) {
                good = true;
                break
            }
        }

        good
        
    }
}

impl Contains_ for String {
    fn contains_(&self, or_tags_group: &Vec<String> ) -> bool {
        let mut good = false;

        for tag in or_tags_group {
            if self.contains(tag) {
                good = true;
                break
            }
        }

        good
    }
}