mod error;
use error::Error;

mod qbit_data;
mod rss;
mod utils;
mod yaml;
use std::time::Duration;

use std::io::prelude::*;
async fn start() -> Result<(), Error> {
    let yaml_data = yaml::FeedManager::from_yaml("config.yaml")?;
    // dbg! {&yaml_data};
    let mut qbit: yaml::QbitMonitor = yaml_data.qbit().await?;

    let feeds = yaml_data.split(&qbit.api);

    feeds.into_iter().for_each(|x| {
        let task = async move {
            loop {
                let tmp = x.run_update().await;
                match tmp {
                    Ok(countdown) => {
                        tokio::time::delay_for(std::time::Duration::from_secs(countdown as u64))
                            .await
                    }
                    Err(e) => {
                        println! {"error fetching torrents: "}
                        dbg! {e};
                        tokio::time::delay_for(std::time::Duration::from_secs(60)).await
                    }
                }
            }
        };
        tokio::spawn(task);
        println! {"spawning new task"};
    });

    loop {
        println! {"looping qbit cycle"};

        if let Err(e) = qbit.pause_all().await {
            println! {"there was an error pausing all public torrents"}
            dbg! {e};
        }

        delay(60).await.await;
    }

    Ok(())
}

async fn delay(interval: u64) -> tokio::time::Delay {
    tokio::time::delay_for(Duration::from_secs(interval))
}
#[tokio::main]
async fn main() {
    start().await;
}
