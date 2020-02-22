use super::rss;
use super::yaml::{QbittorrentAuthentication, RssFeed};
use super::Error;
use qbittorrent::{self, traits::*};
use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::sync::Arc;

use tokio::sync::RwLock;

use reqwest;
use std::boxed::Box;
use std::pin::Pin;

#[derive(Debug)]
pub struct QbitMonitor {
    pub api: Arc<qbittorrent::api::Api>,
    // checked_hashes: HashSet<String>,
    all_hashes: Pin<Box<HashSet<String>>>,
    paused_hashes: HashSet<*const String>,
    trackers: Vec<String>,
}
impl QbitMonitor {
    pub async fn new(qbit_auth: QbittorrentAuthentication) -> Result<Self, Error> {
        let api: qbittorrent::Api =
            qbittorrent::Api::new(&qbit_auth.username, &qbit_auth.password, &qbit_auth.address)
                .await?;
        Ok(Self {
            api: Arc::new(api),
            all_hashes: Pin::new(Box::new(HashSet::new())),
            paused_hashes: HashSet::new(),
            trackers: qbit_auth.trackers,
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
            if self.paused_hashes.contains(&ptr) {
                continue;
            }

            // get all trackers attached to this torrent
            let tracker = match torrent.trackers(&api).await {
                Ok(x) => x,
                Err(e) => {
                    println! {"error getting trackers for torrent {}", torrent.name()};
                    dbg! {e};
                    continue;
                }
            };

            // check each tracker for the torrent against the user-provided list of ok-trackers
            for tracker in tracker {
                // if we we need to seed this tracker then skip tracker
                if self.keep_seeding_tracker(&tracker) {
                    continue;
                }

                // if we get here then we know none of the trackers are ones we care about
                match torrent.pause(&api).await {
                    // the torrent has been successfully paused
                    Ok(_) => {
                        self.paused_hashes.insert(ptr);
                    }
                    Err(e) => {
                        println! {"error pausing torrent: {} ", torrent.name()}
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
        return false;
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
                // tell the client to download the torrent
                if let Ok(_) = self.start_qbit_download(&item).await {
                    // insert it to the history
                    write.insert(item.item_hash);
                } else {
                    dbg! {"failed to download file:", item.title};
                }
            }
        }

        return Ok(self.feed.update_interval);
    }

    // start qbittorrnet's download of a file
    pub async fn start_qbit_download(&self, data: &rss::TorrentData<'_>) -> Result<(), Error> {
        dbg! {"downloading new file"};

        let save_folder = data.original_matcher.save_folder.clone();

        fs::create_dir_all(&save_folder);
        let _x = data.write_metadata();

        let req = qbittorrent::queries::TorrentDownloadBuilder::default()
            .savepath(&save_folder)
            .urls(&data.download_link)
            .category("AUTO_DL")
            .build()
            .expect("incorrect building of download builder");

        self.qbit.add_new_torrent(&req).await?;

        println! {"successfully downloaded new torrent: {}", data.title};
        Ok(())
    }
}
