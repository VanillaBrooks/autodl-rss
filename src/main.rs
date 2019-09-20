mod error;
use error::Error;

mod rss;
mod utils;
mod yaml;
mod qbit_data;

use reqwest;
use std::collections::HashSet;
use std::fs::{self, File};
use std::time;

use std::io;
use std::io::prelude::*;

fn run() -> Result<(), Error> {
    let mut yaml_data = yaml::FeedManager::from_yaml("config.yaml")?;

    dbg!{yaml_data};
    return Ok(());

    loop {
        let update_result = yaml_data.run_update();
        let timer = match update_result {
            Ok(count_down) => std::time::Duration::from_secs(count_down as u64),
            Err(e) => {
                dbg!{e};
                continue
            } 
        };

        yaml_data.clear_public_trackers();
        
        std::thread::sleep(timer);

        break
    }

    Ok(())
}

fn main() {
    // loop {
    let err = run();
    dbg!{err};
    //     dbg! {"ERROR OCCURED: ", err};
    // }
}
