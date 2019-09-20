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

    loop {
        // let new_data = yaml_data.run_update();
        yaml_data.clear_public_trackers();
        break;
    }

    Ok(())
}

fn main() {
    loop {
        let err = run();
        dbg! {"ERROR OCCURED: ", err};
        break;
    }
}
