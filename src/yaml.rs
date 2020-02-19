use super::error::Error;
use super::qbit_data as qbit;
use super::rss;
use super::utils;
use qbittorrent::{self, api::Api, queries, traits::*};

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::sync::Arc;

use tokio::sync::RwLock;

use reqwest;
use serde::{Deserialize, Serialize};
use serde_yaml;

#[derive(Debug, Deserialize)]
pub struct FeedManager {
    feeds: Vec<RssFeed>,
    #[serde(default)]
    next_update: u32,

    // private trackers to keep seeding
    #[serde(default)]
    trackers_to_keep: Vec<String>,

    // qbit hashes that are good and we dont need to recheck
    #[serde(default)]
    good_qbit_hashes: HashSet<String>,

    // qbit hashes that are bad and already paused
    #[serde(default)]
    paused_qbit_hashes: HashSet<String>,

    #[serde(rename = "qbittorrent")]
    qbit_data: QbittorrentAuthentication,
}
impl FeedManager {
    // Fetch yaml of configs to download
    pub fn from_yaml(path: &str) -> Result<FeedManager, Error> {
        let file = File::open(path)?;

        let mut yaml: FeedManager = serde_yaml::from_reader(file)?;
        yaml.lowercase();

        Ok(yaml)
    }

    fn lowercase(&mut self) {
        for i in &mut self.feeds {
            i.lowercase()
        }
    }
    pub async fn qbit(&self) -> Result<QbitMonitor, Error> {
        let qbit = QbitMonitor::new(self.qbit_data.clone()).await?;
        Ok(qbit)
    }

    pub fn split<'a>(self, qbit: &Arc<Api>) -> Vec<FeedMonitor> {
        self.feeds
            .into_iter()
            .map(|x| FeedMonitor::from_feed(x, Arc::clone(&qbit)))
            .collect()
    }
}

#[derive(Debug, Deserialize, Clone)]
struct QbittorrentAuthentication {
    username: String,
    password: String,
    address: String,
    trackers: Vec<String>,
}

#[derive(Debug)]
pub struct QbitMonitor {
    pub api: Arc<qbittorrent::api::Api>,
    paused: HashSet<String>,
    no_pause_hashes: HashSet<String>,
    trackers: Vec<String>,
}
impl QbitMonitor {
    async fn new(qbit_auth: QbittorrentAuthentication) -> Result<Self, Error> {
        let api = Api::new(&qbit_auth.username, &qbit_auth.password, &qbit_auth.address).await?;
        Ok(Self {
            api: Arc::new(api),
            paused: HashSet::new(),
            no_pause_hashes: HashSet::new(),
            trackers: qbit_auth.trackers,
        })
    }

    pub async fn pause_all(&mut self) -> Result<(), Error> {
        let all_torrents: Vec<qbittorrent::data::Torrent> = self.api.get_torrent_list().await?;

        for torrent in &all_torrents {
            // if its not in the tracker list then pause that shit
            if !self.keep_seeding_tracker(torrent) {
                torrent.pause(&self.api).await?
            }
        }
        Ok(())
    }

    // TODO: move this to overall qbit handler

    fn keep_seeding_tracker(&self, t_data: &qbittorrent::data::Torrent) -> bool {
        let mut keep = false;
        for i in &self.trackers {
            if t_data.tracker().contains(i) {
                keep = true
            }
        }
        return keep;
    }
}

#[derive(Debug)]
pub struct FeedMonitor {
    client: reqwest::Client,
    // rss hashes that we have looked at
    previous_hashes: RwLock<HashSet<u64>>,
    feed: RssFeed,
    qbit: Arc<qbittorrent::api::Api>,
}
impl FeedMonitor {
    pub fn from_feed(data: RssFeed, qbit: Arc<qbittorrent::api::Api>) -> Self {
        FeedMonitor {
            client: reqwest::Client::new(),
            previous_hashes: RwLock::new(HashSet::new()),
            feed: data,
            qbit,
        }
    }
    // check all rss feeds for updates: update, pull torrents, and download them if possible
    pub async fn run_update(&self) -> Result<u32, Error> {
        // fetch data from the torrent feed. Error out if there was an issue with the request
        let data = match self.feed.fetch_new(&self.client).await {
            Ok(data) => data,
            Err(e) => return Err(Error::from(e)),
        };

        let mut write = self.previous_hashes.write().await;
        for item in data {
            // if we have not previously downloaded the torrent
            if !write.contains(&item.item_hash) {
                // insert it to the history
                write.insert(item.item_hash);
                // tell the client to download the torrent

                // TODO: start the qbit here
                // self.start_qbit_download(&item).await;
            }
        }

        return Ok(self.feed.update_interval);
    }

    // start qbittorrnet's download of a file
    pub async fn start_qbit_download(&self, data: &rss::TorrentData<'_>) -> Result<(), Error> {
        dbg! {"downloading new file"};

        let save_folder = data.original_matcher.save_folder.clone();

        fs::create_dir_all(&save_folder);
        let x = data.write_metadata();

        let req = qbittorrent::queries::TorrentDownloadBuilder::default()
            .savepath(&save_folder)
            .urls(&data.download_link)
            .build()
            .expect("incorrect building of download builder");

        self.qbit.add_new_torrent(&req).await?;

        println! {"successfully downloaded new torrent: {}", data.title};
        Ok(())
    }
}
use serde_xml_rs as xml;

#[derive(Debug, Deserialize)]
pub struct RssFeed {
    pub url: String,
    pub update_interval: u32,
    #[serde(default)]
    pub last_announce: u32,
    pub matcher: Vec<TorrentMatch>,
}
impl RssFeed {
    pub async fn fetch_new(
        &self,
        pool: &reqwest::Client,
    ) -> Result<Vec<rss::TorrentData<'_>>, Error> {
        let mut response: reqwest::Body = pool.get(&self.url).send().await?.into();

        let data = if let Some(bytes) = response.as_bytes() {
            rss::xml_to_torrents(bytes)?
        } else {
            return Err(Error::SerdeMissing);
        };

        let mut filter_data = data
            .into_iter()
            .map(|mut x| {
                // make sure that the file matches at least one type condition
                let mut data = None;

                for mat in self.matcher.iter() {
                    if mat.match_title(&x.title) && mat.match_tags(&x.tags) {
                        data = Some(rss::TorrentData::from_serde_data(x, mat));
                        break;
                    }
                }

                data
            })
            .filter(|data| data.is_some())
            .map(|x| x.unwrap())
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
        let lower = |arg: &Matcher| match &arg {
            Some(values) => {
                let vals: Vec<Vec<String>> = values
                    .into_iter()
                    .map(|x| x.into_iter().map(|y| y.to_lowercase()).collect())
                    .collect();
                Some(vals)
            }
            None => None,
        };

        self.title_wanted = lower(&self.title_wanted);
        self.title_banned = lower(&self.title_banned);
        self.tags_banned = lower(&self.tags_banned);
        self.tags_wanted = lower(&self.tags_wanted);
    }

    fn match_title(&self, title_input: &String) -> bool {
        // dbg!{title_input};
        // dbg!{&self.title_wanted};
        let mut good_title = true;

        //
        // TODO: make this a better parsing
        //

        if let Some(wanted_titles) = &self.title_wanted {
            for title in wanted_titles {
                if !title_input.contains_(&title) {
                    good_title = false;
                    break;
                }
            }
        }

        if let Some(banned_title) = &self.title_banned {
            for title in banned_title {
                if title_input.contains_(&title) {
                    good_title = false;
                    break;
                }
            }
        }

        // dbg!{good_title};

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
                if !tag_input.contains_(tag) {
                    good_tags = false;
                    break;
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
    fn contains_(&self, or_tags_group: &Vec<String>) -> bool {
        let mut good = false;

        for tag in or_tags_group {
            if self.contains(tag) {
                good = true;
                break;
            }
        }

        good
    }
}

impl Contains_ for String {
    fn contains_(&self, or_tags_group: &Vec<String>) -> bool {
        let mut good = false;

        for tag in or_tags_group {
            if self.contains(tag) {
                good = true;
                break;
            }
        }

        good
    }
}
