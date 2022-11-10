///
/// custom RSS parsing for non-standard rss feeds
///

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use super::yaml;
use super::Error;

use serde::{Deserialize, Serialize};
use serde_yaml;

#[derive(Deserialize, Debug)]
struct Document {
    channel: Option<Channel>,
}

#[derive(Deserialize, Debug)]
struct Channel {
    #[allow(dead_code)]
    title: Option<String>,
    #[allow(dead_code)]
    description: Option<String>,
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
    #[serde(rename = "fileName")]
    file_name: Option<String>,
    #[serde(rename = "infoHash")]
    info_hash: Option<String>,
    #[serde(rename = "contentLength")]
    content_length: Option<u64>,
}

#[derive(Deserialize, Debug, Hash)]
struct Enclosure {
    url: Option<String>,
}

impl Item {
    fn link(&self) -> Result<String, Error> {
        if let Some(enclosure) = &self.enclosure {
            if let Some(url) = &enclosure.url {
                return Ok(url.clone());
            }
        }

        if let Some(link) = &self.link {
            return Ok(link.clone());
        }

        dbg! {"link missing SerdeMissing"};
        Err(Error::SerdeMissing)
    }
}

impl Torrent {
    fn default() -> Self {
        Self {
            file_name: None,
            info_hash: None,
            content_length: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SerdeTorrentData {
    pub title: String,
    pub tags: HashSet<String>,
    pub download_link: String,
    pub size: Option<u64>,
    pub item_hash: u64,
}
impl SerdeTorrentData {
    fn new(mut item: Item) -> Result<Self, Error> {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        let hash = hasher.finish();

        let link = item.link()?;

        let _title = match &item.title {
            Some(title) => title.to_lowercase(),
            None => {
                dbg! {"missing title sending SerdeMissing"};
                return Err(Error::SerdeMissing);
            }
        };
        let tags = match &item.tags {
            Some(tags) => tags
                .split(' ')
                .map(|x| x.to_string().to_lowercase())
                .collect(),
            None => HashSet::new(),
        };
        let torrent = match item.torrent {
            Some(torrent) => torrent,
            None => Torrent::default(),
        };

        Ok(Self {
            title: item.title.take().unwrap().to_lowercase(),
            tags,
            download_link: link,
            size: torrent.content_length,
            item_hash: hash,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct TorrentData<'a> {
    pub title: String,
    pub tags: HashSet<String>,
    pub download_link: String,
    pub size: Option<u64>,
    pub item_hash: u64,
    #[serde(skip)]
    pub original_matcher: &'a yaml::TorrentMatch,
}
impl<'a> TorrentData<'a> {
    pub fn from_serde_data(data: SerdeTorrentData, matcher: &'a yaml::TorrentMatch) -> Self {
        TorrentData {
            title: data.title,
            tags: data.tags,
            download_link: data.download_link,
            size: data.size,
            item_hash: data.item_hash,
            original_matcher: matcher,
        }
    }
    pub fn write_metadata(&self) -> Result<(), Error> {
        let title =
            format! {"{}\\__META_{}.yaml", self.original_matcher.save_folder, self.item_hash};
        let buffer = match std::fs::File::create(&title) {
            Ok(buffer) => buffer,
            Err(e) => {
                println! {"ERROR WHEN WRITING METADATA OF {}:\n\t{}\n\t{}\n\tORIGINAL PATH:{}", self.title, self.original_matcher.save_folder, e, title}
                return Err(Error::from(e));
            }
        };

        let _ser = serde_yaml::to_writer(buffer, &self);

        Ok(())
    }
}

pub fn xml_to_torrents<T: std::io::BufRead>(data: T) -> Result<Vec<SerdeTorrentData>, Error> {
    let doc: Document = quick_xml::de::from_reader(data)?;

    if let Some(channel) = doc.channel {
        if let Some(items) = channel.item {
            let t_data = items
                .into_iter()
                .map(SerdeTorrentData::new)
                .filter_map(|item| item.ok())
                .collect::<Vec<_>>();

            Ok(t_data)
        } else {
            dbg! {"missing channel.item sending serdemissing"};
            Err(Error::SerdeMissing)
        }
    } else {
        dbg! {"missing channel sending sedemissing"};
        Err(Error::SerdeMissing)
    }
}
