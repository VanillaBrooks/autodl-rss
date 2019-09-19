use reqwest;
use rss;
use serde_xml_rs as xml;

#[derive(Debug)]
pub enum Error {
    Reqwest(reqwest::Error),
    Rss(rss::Error),
    Serde(xml::Error),
    SerdeMissing,
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}
impl From<rss::Error> for Error {
    fn from(e: rss::Error) -> Self {
        Error::Rss(e)
    }
}
impl From<xml::Error> for Error {
    fn from(e: xml::Error) -> Self {
        Error::Serde(e)
    }
}
