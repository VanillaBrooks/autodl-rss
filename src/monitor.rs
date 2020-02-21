use super::yaml::{QbittorrentAuthentication, RssFeed};
use super::Error;
use qbittorrent::{self, traits::*};
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;

use super::rss;

use tokio::sync::RwLock;

use reqwest;

#[derive(Debug)]
pub struct QbitMonitor {
    pub api: Arc<qbittorrent::api::Api>,
    paused: HashSet<String>,
    no_pause_hashes: HashSet<String>,
    trackers: Vec<String>,
}
impl QbitMonitor {
    pub async fn new(qbit_auth: QbittorrentAuthentication) -> Result<Self, Error> {
        let api: qbittorrent::Api =
            qbittorrent::Api::new(&qbit_auth.username, &qbit_auth.password, &qbit_auth.address)
                .await?;
        Ok(Self {
            api: Arc::new(api),
            paused: HashSet::new(),
            no_pause_hashes: HashSet::new(),
            trackers: qbit_auth.trackers,
        })
    }

    pub async fn pause_all(&mut self) -> Result<(), Error> {
        // let all_torrents: Vec<qbittorrent::data::Torrent> = self.api.get_torrent_list().await?;

        let all_torrents: Vec<qbittorrent::data::Torrent> =
            qbittorrent::queries::TorrentRequestBuilder::default()
                .filter(qbittorrent::queries::TorrentFilter::Completed)
                .build()
                .expect("torrent request builder error")
                .send(&self.api)
                .await?;

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
