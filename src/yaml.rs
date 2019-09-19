use super::error::Error;
use super::rss;
use super::utils;

use std::collections::HashSet;
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
}
impl FeedManager {
    pub fn from_yaml(path: &str) -> Result<FeedManager, Error> {
        let file = File::open(path)?;

        let mut yaml: FeedManager = serde_yaml::from_reader(file)?;
        yaml.client = Some(reqwest::Client::new());

        Ok(yaml)
    }

    pub fn run_update(&mut self) -> Result<Vec<rss::TorrentData>, Error> {
        let mut next_update_time = 10_000;
        let epoch = utils::current_unix_time();

        let torrent_metadata = self
            .feeds
            .iter()
            .filter(|x| {
                let diff = epoch - x.last_announce;
                if epoch - x.last_announce > x.update_interval {
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
            .collect::<Vec<_>>();

        self.next_update = next_update_time;

        Ok(torrent_metadata)
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
        // dbg!{"sending"}
        // let mut response = pool.get(self.url).send()?;
        // let data = rss::xml_to_torrents(response)?;

        let file = File::open("rarbg.xml").expect("sample file not found");
        let mut data = rss::xml_to_torrents(file)?;
        // dbg!{"made here"};
        // dbg!{data.len()};

        let mut filter_data = data
            .into_iter()
            .map(|mut x| {
                // make sure that the file matches at least one type condition
                let mut condition = false;

                for mat in self.matcher.iter() {
                    if mat.match_title(&x.title) && mat.match_tags(&x.tags) {
                        // dbg!{"found match"};
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

type Matcher = Option<HashSet<String>>;
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
        let mut good_title = true;

        //
        // TODO: make this a better parsing
        //

        if let Some(wanted_titles) = &self.title_wanted {
            for title in wanted_titles {
                // println!{"checking {} for key work {}", title_input, title}
                if !title_input.contains(title) {
                    // println!{"does not contain title KW, quitting"}
                    good_title = false;
                    break
                }
            }
        }

        if let Some(banned_title) = &self.title_banned {
            for title in banned_title {
                if title_input.contains(title) {
                    good_title = false;
                }
            }
        }

        return good_title;
    }

    // make sure the HashSet is all lowercase
    fn match_tags(&self, tag_input: &HashSet<String>) -> bool {
        let mut good_tags = true;

        //
        // TODO: make this a better parsing
        //

        if let Some(tags_wanted) = &self.tags_wanted {
            for tag in tags_wanted {
                if !tag_input.contains(tag) {
                    good_tags = false
                }
            }
        }

        if let Some(banned_tags) = &self.tags_banned {
            for tag in banned_tags {
                if tag_input.contains(tag) {
                    good_tags = false;
                }
            }
        }

        return good_tags;
    }
}
