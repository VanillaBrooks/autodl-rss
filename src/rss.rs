use super::error::Error;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{self, Hash, Hasher};
// custom RSS parsing for non-standard rss feeds

use serde_xml_rs as xml;

#[derive(Deserialize, Debug)]
struct Document {
    channel: Option<Channel>,
}

#[derive(Deserialize, Debug)]
struct Channel {
    title: Option<String>,
    description: Option<String>,
    link: Option<String>,
    item: Option<Vec<Item>>,
}

#[derive(Deserialize, Debug, Hash)]
struct Item {
    title: Option<String>,
    link: Option<String>,
    tags: Option<String>,
    torrent: Option<Torrent>,
    enclosure: Option<Enclosure>,
}

#[derive(Deserialize, Debug, Hash)]
struct Torrent {
    fileName: Option<String>,
    infoHash: Option<String>,
    contentLength: Option<u64>,
}

#[derive(Deserialize, Debug, Hash)]
struct Enclosure {
    url: Option<String>
}

impl Item {
    fn link(&self) -> Result<String, Error> {
        if let Some(enclosure) = &self.enclosure {
            if let Some(url) = &enclosure.url {
                return Ok(url.clone())
            }
        }

        if let Some(link) = &self.link {
            return Ok(link.clone())
        }

        return Err(Error::SerdeMissing)
    }
}

pub fn test() {
    dbg! {"in test function"};

    let file = std::fs::File::open("test_xml.xml").expect("could not open xml");

    let data: Document = xml::from_reader(file).unwrap();

    dbg! {&data};
}

#[derive(Debug)]
pub struct TorrentData {
    pub title: String,
    pub tags: HashSet<String>,
    pub download_link: String,
    pub size: Option<u64>,
    pub item_hash: u64,
}
impl TorrentData {
    fn new(item: Item) -> Result<Self, Error> {
        match &item.title {
            Some(title) => match &item.tags {
                Some(tags) => match &item.torrent {
                    Some(torrent) => {
                        let mut hasher = DefaultHasher::new();
                        item.hash(&mut hasher);
                        let item_hash = hasher.finish();

                        Ok(Self {
                            title: title.to_string(),
                            tags: tags.split(" ").map(|x| x.to_string()).collect(),
                            download_link: item.link()?,
                            size: torrent.contentLength,
                            item_hash: item_hash,
                        })
                    }
                    None => Err(Error::SerdeMissing),
                },
                None => Err(Error::SerdeMissing),
            },
            None => Err(Error::SerdeMissing),
        }
    }
}

pub fn xml_to_torrents<T: std::io::Read>(data: T) -> Result<Vec<TorrentData>, Error> {
    let doc: Document = xml::from_reader(data)?;

    // dbg!{&doc};

    if let Some(channel) = doc.channel {
        if let Some(items) = channel.item {
            let t_data = items
                .into_iter()
                .map(|item| TorrentData::new(item))
                .filter(|item| item.is_ok())
                .map(|item| item.unwrap())
                .collect::<Vec<_>>();

            Ok(t_data)
        } else {
            Err(Error::SerdeMissing)
        }
    } else {
        Err(Error::SerdeMissing)
    }
}
