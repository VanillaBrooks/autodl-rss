mod error;
use error::Error;

mod qbit_data;
mod rss;
mod utils;
mod yaml;

use reqwest;
use std::collections::HashSet;
use std::fs::{self, File};
use std::time;

use std::io;
use std::io::prelude::*;

fn run() -> Result<(), Error> {
    let mut yaml_data = yaml::FeedManager::from_yaml("config.yaml")?;

    dbg! {&yaml_data};

    loop {
        let update_result = yaml_data.run_update();
        let timer = match update_result {
            Ok(count_down) => std::time::Duration::from_secs(count_down as u64),
            Err(e) => {
                dbg! {e};
                continue;
            }
        };

        yaml_data.clear_public_trackers();

        println! {"sleeping for {:?}, current_time: {}", timer, utils::current_unix_time()};

        std::thread::sleep(timer);
    }

    Ok(())
}

fn main() {
    dbg! {run()};
    // dbg!{std::time::Instant::now()};
}
