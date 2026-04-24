use chrono::{Datelike, Local};

pub fn current_timestamp() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn current_year() -> u32 {
    Local::now().year().max(0) as u32
}
