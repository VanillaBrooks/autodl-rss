pub mod monitor;
pub mod rss;
pub mod yaml;

#[derive(Debug, thiserror::Error )]
pub enum Error {
    #[error("")]
    Reqwest(#[from] reqwest::Error),
    //#[error("")]
    //Serde(#[from] serde_xml_rs::Error),
    #[error("")]
    Serde(#[from] quick_xml::DeError),
    #[error("")]
    IoError(#[from] std::io::Error),
    #[error("")]
    YamlError(#[from] serde_yaml::Error),
    #[error("")]
    JsonError(#[from] serde_json::Error),
    #[error("")]
    SerdeGeneral,
    #[error("")]
    QbitError(#[from] qbittorrent::error::Error),
    #[error("")]
    SerdeMissing,
    #[error("")]
    MissingBytes,
    #[error("")]
    InvalidHeader(#[from] http::header::InvalidHeaderValue),
    #[error("the configuration file was missing from all locations")]
    ConfigMissing,
}
