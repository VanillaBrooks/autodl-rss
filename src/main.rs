use std::time::Duration;

use autodl_rss;
use autodl_rss::{monitor, yaml, Error};

async fn start() -> Result<(), Error> {
    let yaml_data = yaml::FeedManager::from_yaml(&["/config/config.yaml", "config.yaml"])?;
    println! {"opened config.yaml"}
    let mut qbit: monitor::QbitMonitor = yaml_data.qbit().await?;

    let feeds = yaml_data.split(&qbit.api);

    feeds.into_iter().for_each(|mut x| {
        let task = async move {
            loop {
                let tmp = x.run_update().await;
                match tmp {
                    Ok(countdown) => {
                        println! {"Finished RSS update for tracker: {}", x.feed().url};
                        tokio::time::delay_for(std::time::Duration::from_secs(countdown as u64))
                            .await
                    }
                    Err(e) => {
                        println! {"main thread error fetching torrents: {:?}", e}
                        tokio::time::delay_for(std::time::Duration::from_secs(60)).await
                    }
                }
            }
        };
        tokio::spawn(task);
        println! {"spawning new task"};
    });

    loop {
        println! {"looping through qbittorrent checks"};

        // get a list of all hashes
        if let Err(e) = qbit.sync_qbit().await {
            println! {"error getting full torrent list hashes"}
            dbg! {e};
        }

        // pause all torrents from trackers not matching
        if let Err(e) = qbit.pause_all().await {
            println! {"there was an error pausing all public torrents"}
            dbg! {e};
        }

        // pause all torrents with titles we do not want
        if let Err(e) = qbit.check_titles().await {
            println! {"there was an error checking torrent titles"}
            dbg! {e};
        }

        delay(60).await.await;
    }
}

async fn delay(interval: u64) -> tokio::time::Delay {
    tokio::time::delay_for(Duration::from_secs(interval))
}
#[tokio::main]
async fn main() {
    println! {"sleeping for 10 seconds"}
    std::thread::sleep(std::time::Duration::from_secs(10));
    dbg! {start().await};
    dbg! {"could not start downloader"};
}
