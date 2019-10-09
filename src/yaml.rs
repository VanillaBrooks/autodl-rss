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
        yaml.lowercase();
        yaml.client = Some(reqwest::Client::new());

        Ok(yaml)
    }

    // check all rss feeds for updates: update, pull torrents, and download them if possible
    pub fn run_update(&mut self) -> Result<u32, Error> {
        let mut next_update_time = 60 * 60;
        let epoch = utils::current_unix_time();

        let mut hashes_to_add = HashSet::new();

        self.feeds
            .iter()
            .filter(|x| {
                // the time between the last time we parsed the RSS feed and now
                let diff = epoch - x.last_announce;

                // if the number of seconds since last update is greater than the number 
                // of seconds that we wait between updates we will update the RSS feed 
                if epoch - x.last_announce > x.update_interval {

                    // if the time to the next update is smaller than the current 
                    // greatest time to update we change the next update interval to
                    // correspond to this RSS feed
                    if x.update_interval < next_update_time {
                        next_update_time = x.update_interval
                    }

                    true

                // else: this RSS feed should not be updated yet
                } else {
                    // I comment this out since i updated other parts of the code
                    // and i think this now breaks things
                    // if diff < next_update_time {
                        // next_update_time = diff
                    // }
                    false
                }
            })
            // for each RSS feed that needs updating, update it
            .map(|x| x.fetch_new(&self.client.as_ref().unwrap()))
            // if the rss parsing is Result::Ok()
            .filter(|x| x.is_ok())
            // unwrap good results
            .map(|x| x.unwrap())
            // flatten nested vectors to one vector
            .flatten()
            // send data to qbittorrent
            .for_each(|data| {
                // if we have not previously sent this to qbit...
                if !self.previous_hashes.contains(&data.item_hash) {
                    self.start_qbit_download(&data);
                    hashes_to_add.insert(data.item_hash);
                }
            });

        // insert current hashes into the list of hashes that do not need to be checked in the future
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
        // post.insert("sequentialDownload", "true".to_string());

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
        dbg!{"clearing public trackers"};
        let cref = self.client.as_ref().unwrap();

        let ans = cref
            .get("http://localhost:8080/query/torrents?filter=completed")
            .send()?;

        let data = qbit::QbitData::from_reader(ans)?;
        for torrent in &data {

            if !self.good_qbit_hashes.contains(&torrent.hash) && !self.paused_qbit_hashes.contains(&torrent.hash) {
                
                let request = format!{"http://localhost:8080/query/propertiesTrackers/{}", &torrent.hash};
                dbg!{&request};
                let mut trackers = cref.get(&request)
                .send()?;
                
                let data=  qbit::TrackerData::from_reader(trackers);

                let specific_torrent_data = match data{
                    Ok(data) => data,
                    Err(_) => {println!{"continue"};continue}
                };
            
                // the torrent is in an approved tracker. save the hash so we dont check latter
                if self.keep_seeding_tracker(&specific_torrent_data) {
                    self.good_qbit_hashes.insert(torrent.hash.clone());
                    dbg!{"2.1"};
                }
                // stop the torrent since its completed
                else{

                    dbg!{"stopping torrent"};

                    let mut map = reqwest::header::HeaderMap::new();
                    map.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Fiddler"));

                    dbg!{"2.2"};
                    let mut form = HashMap::new();
                    form.insert("hash", torrent.hash.to_string());

                    let command_url = format!{"http://localhost:8080/command/pause?hash={}",torrent.hash};
                    let command_url = "http://localhost:8080/command/pause";
                    let response = cref
                        .post(command_url)
                        .headers(map)
                        .form(&form)
                        .send();

                    dbg!{"2.3"};
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

    fn lowercase(&mut self) {
        for i in &mut self.feeds {
            i.lowercase()
        }
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

    fn lowercase(&mut self) {
        for j in &mut self.matcher {
            j.lowercase()
        }
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
    fn lowercase(&mut self) {
        let lower = |arg: &Matcher| {
            match &arg {
                Some(values) => {
                    let vals: Vec<Vec<String>> = 
                        values.into_iter().map(|x|{
                            x.into_iter().map(|y| y.to_lowercase()).collect()
                        })
                        .collect();
                    Some(vals)
                },
                None => None
            }
        };

        self.title_wanted = lower(&self.title_wanted);
        self.title_banned = lower(&self.title_banned);
        self.tags_banned = lower(&self.tags_banned);
        self.tags_wanted = lower(&self.tags_wanted);
    }

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