mod error;
use error::Error;

mod qbit_data;
mod rss;
mod utils;
mod yaml;

use reqwest;
use std::collections::HashSet;
use std::fs::{self, File};
use std::sync::Arc;
use std::time;

use std::io;
use std::io::prelude::*;

async fn run() -> Result<(), Error> {
    let mut yaml_data = yaml::FeedManager::from_yaml("config.yaml")?;
    let mut qbit: yaml::QbitMonitor = yaml_data.qbit().await?;

    let feeds = yaml_data.split(&qbit.api);

    feeds.into_iter().map(|x| {
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
    });

    tokio::spawn(async move {
        loop {
            qbit.pause_all().await;
        }
    });

    Ok(())
}

#[tokio::main]
async fn main() {
    dbg! {run().await};
    // dbg!{std::time::Instant::now()};
}
