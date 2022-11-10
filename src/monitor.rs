use super::rss;
use super::yaml::{QbittorrentAuthentication, RssFeed};
use super::Error;
use qbittorrent::{self, traits::*};
use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::sync::Arc;

use reqwest;

const AUTODL_CATEGORY: &str = "AUTO_DL";
const TITLE_BAN_CATEGORY: &str = "TITLE_BAN";

#[derive(Debug)]
pub struct QbitMonitor {
    pub api: Arc<qbittorrent::api::Api>,
    // checked_hashes: HashSet<String>,
    all_hashes: HashSet<String>,
    // paused due to tracker requirements
    paused_tracker_hashes: HashSet<*const String>,
    // paused do to title issues
    paused_title_hashes: HashSet<*const String>,
    trackers: Vec<String>,
    title_bans: Vec<String>,
    #[allow(dead_code)]
    file_bans: Vec<String>,
}

impl QbitMonitor {
    pub async fn new(qbit_auth: QbittorrentAuthentication) -> Result<Self, Error> {
        let api: qbittorrent::Api =
            qbittorrent::Api::new(&qbit_auth.username, &qbit_auth.password, &qbit_auth.address)
                .await?;

        // set up category for torrents that do not meet title criteria
        api.add_category(TITLE_BAN_CATEGORY, "").await?;

        let title_bans = qbit_auth.title_bans.unwrap_or_default();
        let file_bans = qbit_auth.file_bans.unwrap_or_default();

        let lower = |x: Vec<String>| x.into_iter().map(|x| x.to_ascii_lowercase()).collect();
        let title_bans = lower(title_bans);
        let file_bans = lower(file_bans);
        let trackers = lower(qbit_auth.trackers);

        Ok(Self {
            api: Arc::new(api),
            all_hashes: HashSet::new(),
            paused_tracker_hashes: HashSet::new(),
            paused_title_hashes: HashSet::new(),
            trackers,
            title_bans,
            file_bans,
        })
    }

    pub async fn sync_qbit(&mut self) -> Result<(), Error> {
        let all_torrents: Vec<qbittorrent::data::Torrent> =
            qbittorrent::queries::TorrentRequestBuilder::default()
                .build()
                .expect("torrent request builder error")
                .send(&self.api)
                .await?;

        all_torrents.iter().for_each(|new_torrent| {
            self.all_hashes
                .insert(new_torrent.hash().deref().to_string());
        });

        Ok(())
    }

    pub async fn pause_all(&mut self) -> Result<(), Error> {
        let all_torrents: Vec<qbittorrent::data::Torrent> =
            qbittorrent::queries::TorrentRequestBuilder::default()
                .filter(qbittorrent::queries::TorrentFilter::Completed)
                .build()
                .expect("torrent request builder error")
                .send(&self.api)
                .await?;

        let api = &self.api;

        for torrent in all_torrents {
            // get a pointer to some item in the hashset
            let ptr = if let Some(hash) = self.all_hashes.get(torrent.hash().deref()) {
                hash as *const String
            } else {
                println! {"hash was not in all hashes as expected for torrent: {}", torrent.name()}
                continue;
            };

            // if we have the pointer stored then we have paused the torrent previously
            if self.paused_tracker_hashes.contains(&ptr) {
                continue;
            }

            // get all trackers attached to this torrent
            let tracker = match torrent.trackers(api).await {
                Ok(x) => x,
                Err(e) => {
                    println! {"error getting trackers for torrent {}", torrent.name()};
                    dbg! {e};
                    continue;
                }
            };

            let mut pause_torrent = true;
            // check each tracker for the torrent against the user-provided list of ok-trackers
            for tracker in tracker {
                // if we we need to seed this tracker then skip tracker
                if self.keep_seeding_tracker(&tracker) {
                    pause_torrent = false;
                }
            }

            // if we never found a tracker that we allowed then pause_torrent is true
            // and we send the command to stop seeding
            if pause_torrent {
                // if we get here then we know none of the trackers are ones we care about
                match torrent.pause(api).await {
                    // the torrent has been successfully paused
                    Ok(_) => {
                        self.paused_tracker_hashes.insert(ptr);
                    }
                    Err(e) => {
                        println! {"error pausing torrent for tracker reasons: {} ", torrent.name()}
                        dbg! {e};
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn check_titles(&mut self) -> Result<(), Error> {
        // if we have no title bans just quit
        if self.title_bans.is_empty() {
            return Ok(());
        }

        // fetch all torrents that have been automatically downloaded
        let all_torrents: Vec<qbittorrent::data::Torrent> =
            qbittorrent::queries::TorrentRequestBuilder::default()
                .filter(qbittorrent::queries::TorrentFilter::All)
                .category(AUTODL_CATEGORY)
                .build()
                .expect("torrent request builder error")
                .send(&self.api)
                .await?;

        for torrent in all_torrents {
            // println! {"testing to pause torrent name: {}", torrent.name()};

            // get a pointer to some item in the hashset
            let ptr = if let Some(hash) = self.all_hashes.get(torrent.hash().deref()) {
                hash as *const String
            } else {
                println! {"hash was not in all hashes as expected for torrent: {}", torrent.name()}
                continue;
            };

            // if we have the pointer stored then we have paused the torrent previously
            if self.paused_title_hashes.contains(&ptr) {
                continue;
            }

            // check if the title is acceptable
            if !self.torrent_title_acceptable(&torrent) {
                // if we get here then we know we need to pause things
                match torrent.set_category(&self.api, TITLE_BAN_CATEGORY).await {
                    // the torrent has been successfully paused
                    Ok(_) => {
                        if torrent.pause(&self.api).await.is_ok() {
                            self.paused_title_hashes.insert(ptr);
                        }
                    }
                    Err(e) => {
                        println! {"error setting title ban category for torrent: {} ", torrent.name()}
                        dbg! {e};
                    }
                }
            }
        }

        Ok(())
    }

    fn keep_seeding_tracker(&self, t_data: &qbittorrent::data::Tracker) -> bool {
        for i in &self.trackers {
            if t_data.url().contains(i) {
                return true;
            }
        }

        false
    }

    fn torrent_title_acceptable(&self, t_data: &qbittorrent::data::Torrent) -> bool {
        for i in &self.title_bans {
            if t_data.name().to_ascii_lowercase().contains(i) {
                return false;
            }
        }

        true
    }
}

#[derive(Debug)]
pub struct FeedMonitor {
    client: reqwest::Client,
    // rss hashes that we have looked at
    previous_hashes: HashSet<u64>,
    feed: RssFeed,
    qbit: Arc<qbittorrent::api::Api>,
}
impl FeedMonitor {
    pub fn from_feed(data: RssFeed, qbit: Arc<qbittorrent::api::Api>) -> Self {
        FeedMonitor {
            client: reqwest::Client::new(),
            previous_hashes: HashSet::new(),
            feed: data,
            qbit,
        }
    }
    // check all rss feeds for updates: update, pull torrents, and download them if possible
    pub async fn run_update(&mut self) -> Result<u32, Error> {
        // fetch data from the torrent feed. Error out if there was an issue with the request
        let data = match self.feed.fetch_new(&self.client).await {
            Ok(data) => data,
            Err(e) => return Err(e),
        };

        // let mut write = &mut self.previous_hashes;

        for item in data {
            // if we have not previously downloaded the torrent
            if !self.previous_hashes.contains(&item.item_hash) {
                // tell the client to download the torrent
                if self.start_qbit_download(&item).await.is_ok() {
                    // insert it to the history
                    // write.insert(item.item_hash);
                    self.previous_hashes.insert(item.item_hash);
                } else {
                    dbg! {"failed to download file:", item.title};
                }
            }
        }

        Ok(self.feed.update_interval)
    }

    pub fn feed(&self) -> &RssFeed {
        &self.feed
    }

    // start qbittorrnet's download of a file
    pub async fn start_qbit_download(&self, data: &rss::TorrentData<'_>) -> Result<(), Error> {
        dbg! {"downloading new file"};

        let save_folder = data.original_matcher.save_folder.clone();

        if let Err(e) = fs::create_dir_all(&save_folder) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(Error::from(e));
            }
        }

        let _x = data.write_metadata();

        let req = qbittorrent::queries::TorrentDownloadBuilder::default()
            .savepath(&save_folder)
            .urls(&data.download_link)
            .paused(data.original_matcher.start_condition())
            .category(AUTODL_CATEGORY)
            .build()
            .expect("incorrect building of download builder");

        self.qbit.add_new_torrent(&req).await?;

        println! {"successfully downloaded new torrent: {}", data.title};
        Ok(())
    }
}
