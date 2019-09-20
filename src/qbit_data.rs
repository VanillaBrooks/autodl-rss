use serde::{Deserialize};
use super::error::Error;
use serde_json;
use std::io::prelude::*;

#[derive(Debug, Deserialize)]
pub struct QbitData {
    dlspeed: i32,
    eta: i32,
    f_l_piece_prio: bool,
    force_start: bool,
    pub hash: String,
    category: String,
    name: String,
    num_complete: i32,
    num_incomplete: i32,
    num_leechs: i32,
    num_seeds: i32,
    priority: i32,
    progress: f32,
    ratio: f32,
    seq_dl: bool,
    size: usize,
    state: String,
    super_seeding: bool,
    upspeed: i32 
}

impl QbitData {
    pub fn from_reader<T:Read>(r: T) -> Result<Vec<Self>, Error>{
        let data : Vec<Self>= serde_json::from_reader(r)?;

        Ok(data)
    }
}

#[derive(Deserialize, Debug)]
pub struct TrackerData {
    msg: String,
    num_peers: u32,
    status: String,
    url: String,
}
impl TrackerData {
    pub fn from_reader<T: Read>(r:T) -> Result<Self, Error> {
        let mut data : Vec<Self> = serde_json::from_reader(r)?;
        let data = data.remove(0);

        Ok(data)
    }
    pub fn url(&self) -> &String {
        &self.url
    }
}