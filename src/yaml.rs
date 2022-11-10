use super::Error;

use super::rss;

use qbittorrent;

use std::collections::HashSet;
use std::sync::Arc;

use super::monitor::*;
use reqwest;
use serde::Deserialize;
use serde_yaml;

#[derive(Debug, Deserialize)]
pub struct FeedManager {
    feeds: Vec<RssFeed>,

    #[serde(rename = "qbittorrent")]
    qbit_data: QbittorrentAuthentication,
}
impl FeedManager {
    // Fetch yaml of configs to download
    pub fn from_yaml(paths: &[&str]) -> Result<FeedManager, Error> {
        // cycle through all the paths, figure if any of the file locations are valid
        let mut iter = paths
            .iter()
            .map(|file_location| (file_location, std::fs::File::open(file_location)))
            .filter(|(_file_loc, file_opening)| file_opening.is_ok())
            .map(|(file_location, file)| (file_location, file.unwrap()));

        let file = if let Some((filename, file)) = iter.next() {
            println! {"FeedManager: using file from : {}", filename}
            file
        } else {
            return Err(Error::ConfigMissing);
        };

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

    pub fn split(self, qbit: &Arc<qbittorrent::Api>) -> Vec<FeedMonitor> {
        self.feeds
            .into_iter()
            .map(|x| FeedMonitor::from_feed(x, Arc::clone(qbit)))
            .collect()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct QbittorrentAuthentication {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) address: String,
    pub(crate) trackers: Vec<String>,
    pub(crate) title_bans: Option<Vec<String>>,
    pub(crate) file_bans: Option<Vec<String>>,
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
    pub async fn fetch_new(
        &self,
        pool: &reqwest::Client,
    ) -> Result<Vec<rss::TorrentData<'_>>, Error> {
        let mut header = reqwest::header::HeaderMap::with_capacity(1);
        header.insert(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 6.1; WOW64; Trident/7.0; rv:11.0) like Gecko".parse()?,
        );

        let response: &[u8] = &pool
            .get(&self.url)
            .headers(header)
            .send()
            .await?
            .bytes()
            .await?;

        let data = rss::xml_to_torrents(response)?;

        let filter_data = data
            .into_iter()
            .filter_map(|x| {
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
    pub start_paused: Option<bool>,
}
impl TorrentMatch {
    fn lowercase(&mut self) {
        let lower = |arg: &Matcher| match &arg {
            Some(values) => {
                let vals: Vec<Vec<String>> = values
                    .iter()
                    .map(|x| x.iter().map(|y| y.to_lowercase()).collect())
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
        let mut good_title = true;

        //
        // TODO: make this a better parsing
        //

        if let Some(wanted_titles) = &self.title_wanted {
            for title in wanted_titles {
                if !title_input.contains_(title) {
                    good_title = false;
                    break;
                }
            }
        }

        if let Some(banned_title) = &self.title_banned {
            for title in banned_title {
                if title_input.contains_(title) {
                    good_title = false;
                    break;
                }
            }
        }

        good_title
    }

    // make sure the HashSet is all lowercase
    fn match_tags(&self, tag_input: &HashSet<String>) -> bool {
        let mut good_tags = true;
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

        good_tags
    }

    pub(crate) fn start_condition(&self) -> String {
        let x = match self.start_paused {
            Some(val) => match val {
                true => "true",
                false => "false",
            },
            None => "false",
        };
        x.into()
    }
}

trait Contains_ {
    fn contains_(&self, value: &[String]) -> bool;
}

impl Contains_ for HashSet<String> {
    fn contains_(&self, or_tags_group: &[String]) -> bool {
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
    fn contains_(&self, or_tags_group: &[String]) -> bool {
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
