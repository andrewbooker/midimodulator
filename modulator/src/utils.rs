use std::time::SystemTime;
use chrono::prelude::{DateTime, Utc};

pub fn today() -> String {
    let now: DateTime<Utc> = SystemTime::now().into();
    now.format("%Y-%m-%d").to_string()
}
