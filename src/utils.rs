use std::time;

pub fn current_unix_time() -> u32 {
    return time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
}
