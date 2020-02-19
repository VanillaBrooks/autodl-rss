use qbittorrent;
use reqwest;
use serde_json as json;
use serde_xml_rs as xml;
use serde_yaml as yaml;

type qb = qbittorrent::error::Error;
#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    Serde(xml::Error),
    SerdeMissing,
    IoError(std::io::Error),
    YamlError(yaml::Error),
    JsonError(json::Error),
    SerdeGeneral,
    QbitError(qb),
    MissingBytes,
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}
// impl From<rss::Error> for Error {
//     fn from(e: rss::Error) -> Self {
//         Error::Rss(e)
//     }
// }
impl From<xml::Error> for Error {
    fn from(e: xml::Error) -> Self {
        Error::Serde(e)
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IoError(e)
    }
}
impl From<yaml::Error> for Error {
    fn from(e: yaml::Error) -> Self {
        Error::YamlError(e)
    }
}
impl From<json::Error> for Error {
    fn from(e: json::Error) -> Self {
        Error::JsonError(e)
    }
}
impl From<qb> for Error {
    fn from(e: qb) -> Self {
        Error::QbitError(e)
    }
}
