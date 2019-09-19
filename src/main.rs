mod error;
use error::Error;

mod rss;

use reqwest;
use std::collections::HashSet;
use std::fs::{self, File};
use std::time;

use std::io;
use std::io::prelude::*;

struct RssFeed<'a> {
    url: &'a str,
    minute_interval: u32,
    last_announce: u32,
    matcher: TorrentMatch,
}
impl<'a> RssFeed<'a> {
    // fix me here
    // fix me here
    // fix me here
    fn from_yaml(yaml: String) -> Self {
        unimplemented!()
    }

    fn fetch_new(&self, pool: &reqwest::Client) -> Result<Vec<rss::TorrentData>, Error> {
        // dbg!{"sending"}
        // let mut response = pool.get(self.url).send()?;
        // let data = rss::xml_to_torrents(response)?;

        let file = File::open("test_xml.xml").expect("sample file not found");
        let data = rss::xml_to_torrents(file)?;

        let filter_data = data
            .into_iter()
            .filter(|x| self.matcher.match_title(&x.title) && self.matcher.match_tags(&x.tags))
            .collect::<Vec<_>>();

        Ok(filter_data)
    }
}

type VecStr = Vec<String>;
struct TorrentMatch {
    title_wanted: VecStr,
    title_banned: VecStr,

    tags_wanted: VecStr,
    tags_banned: VecStr,
}
impl TorrentMatch {
    fn new(t_w: VecStr, t_b: VecStr, tag_w: VecStr, tag_b: VecStr) -> Self {
        Self {
            title_wanted: t_w,
            title_banned: t_b,
            tags_wanted: tag_w,
            tags_banned: tag_b,
        }
    }

    fn match_title(&self, title_input: &String) -> bool {
        let mut good_title = true;

        //
        // TODO: make this a better parsing
        //

        for title in &self.title_wanted {
            if !title_input.contains(title) {
                good_title = false
            }
        }

        for title in &self.title_banned {
            if title_input.contains(title) {
                good_title = false;
            }
        }

        return good_title;
    }

    // make sure the HashSet is all lowercase
    fn match_tags(&self, tag_input: &HashSet<String>) -> bool {
        let mut good_tags = true;

        //
        // TODO: make this a better parsing
        //

        for tag in &self.tags_wanted {
            if !tag_input.contains(tag) {
                good_tags = false
            }
        }

        for tag in &self.tags_banned {
            if tag_input.contains(tag) {
                good_tags = false;
            }
        }

        return good_tags;
    }
}

fn init_setup() {
    fs::create_dir("temp");
}

fn private_trackers() {}

fn main() {
    let feed = RssFeed {
        url: "",
        minute_interval: 0,
        last_announce: 0,
        matcher: TorrentMatch::new(vec![], vec![], vec![], vec![]),
    };

    let client = reqwest::Client::new();
    feed.fetch_new(&client);

    // dbg! {"here"};
    // rss::test();
}

fn current_unix_time() -> u32 {
    return time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
}
