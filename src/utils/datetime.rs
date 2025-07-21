use std::time::SystemTime;

use chrono::{DateTime, Utc};

#[allow(clippy::cast_possible_wrap)]
pub fn system_time_to_datetime(time: std::io::Result<SystemTime>) -> Option<DateTime<Utc>> {
    time.ok().and_then(|t| {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
    })
}
