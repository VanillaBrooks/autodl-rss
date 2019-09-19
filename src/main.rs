mod error;
use error::Error;

mod rss;
mod utils;
mod yaml;

use reqwest;
use std::collections::HashSet;
use std::fs::{self, File};
use std::time;

use std::io;
use std::io::prelude::*;

fn init_setup() {
    fs::create_dir("temp");
}

fn private_trackers() {}

fn main() {
    // let feed = yaml::RssFeed {
    //     url: "".to_string(),
    //     minute_interval: 0,
    //     last_announce: 0,
    //     matcher: yaml::TorrentMatch::new(vec![], vec![], vec![], vec![]),
    // };

    // let client = reqwest::Client::new();
    // feed.fetch_new(&client);

    let yaml_data = yaml::FeedManager::from_yaml("config.yaml");

    // dbg! {&yaml_data};

    let mut yaml_data = yaml_data.unwrap();

    let new_data = yaml_data.run_update();


    dbg!{new_data};
}
