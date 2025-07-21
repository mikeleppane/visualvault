use std::time::SystemTime;

use chrono::{DateTime, Utc};

#[allow(clippy::cast_possible_wrap)]
#[must_use]
pub fn system_time_to_datetime(time: std::io::Result<SystemTime>) -> Option<DateTime<Utc>> {
    time.ok().and_then(|t| {
        t.duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|d| DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos()))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]
    #![allow(clippy::float_cmp)] // For comparing floats in tests
    #![allow(clippy::panic)]
    use super::*;
    use std::io;
    use std::time::Duration;

    #[test]
    fn test_system_time_to_datetime_valid_time() {
        // Test with a valid system time
        let system_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let result = system_time_to_datetime(Ok(system_time));

        assert!(result.is_some());
        let datetime = result.unwrap();

        // 1_000_000_000 seconds after UNIX_EPOCH is September 9, 2001
        assert_eq!(datetime.timestamp(), 1_000_000_000);
        assert_eq!(datetime.format("%Y-%m-%d").to_string(), "2001-09-09");
    }

    #[test]
    fn test_system_time_to_datetime_unix_epoch() {
        // Test with UNIX_EPOCH itself
        let result = system_time_to_datetime(Ok(SystemTime::UNIX_EPOCH));

        assert!(result.is_some());
        let datetime = result.unwrap();

        assert_eq!(datetime.timestamp(), 0);
        assert_eq!(datetime.format("%Y-%m-%d %H:%M:%S").to_string(), "1970-01-01 00:00:00");
    }

    #[test]
    fn test_system_time_to_datetime_with_nanoseconds() {
        // Test with nanosecond precision
        let nanos = 123_456_789u32;
        let system_time = SystemTime::UNIX_EPOCH + Duration::new(1_500_000_000, nanos);
        let result = system_time_to_datetime(Ok(system_time));

        assert!(result.is_some());
        let datetime = result.unwrap();

        assert_eq!(datetime.timestamp(), 1_500_000_000);
        assert_eq!(datetime.timestamp_subsec_nanos(), nanos);
    }

    #[test]
    fn test_system_time_to_datetime_io_error() {
        // Test with IO error
        let io_error = io::Error::other("test error");
        let result = system_time_to_datetime(Err(io_error));

        assert!(result.is_none());
    }

    #[test]
    fn test_system_time_to_datetime_before_unix_epoch() {
        // Test with time before UNIX_EPOCH (should return None)
        let system_time = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
        let result = system_time_to_datetime(Ok(system_time));

        // This should return None because duration_since will fail
        assert!(result.is_none());
    }

    #[test]
    fn test_system_time_to_datetime_current_time() {
        // Test with current time
        let now = SystemTime::now();
        let result = system_time_to_datetime(Ok(now));

        assert!(result.is_some());
        let datetime = result.unwrap();

        // Verify the time is recent (within last minute)
        let current_timestamp = Utc::now().timestamp();
        assert!((datetime.timestamp() - current_timestamp).abs() < 60);
    }

    #[test]
    fn test_system_time_to_datetime_max_duration() {
        // Test with maximum safe duration
        // i64::MAX seconds is about 292 billion years
        let max_safe_seconds = i64::MAX as u64;
        let system_time = SystemTime::UNIX_EPOCH + Duration::from_secs(max_safe_seconds);
        let result = system_time_to_datetime(Ok(system_time));

        // This might fail due to overflow in from_timestamp
        // The function should handle this gracefully
        if let Some(datetime) = result {
            assert!(datetime.timestamp() > 0);
        }
    }

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_system_time_to_datetime_specific_dates() {
        // Test with specific known dates
        let test_cases = vec![
            (946_684_800, "2000-01-01"),   // Y2K
            (1_234_567_890, "2009-02-13"), // Unix time 1234567890
            (1_609_459_200, "2021-01-01"), // 2021 New Year
            (2_147_483_647, "2038-01-19"), // 32-bit signed int max (Y2038 problem)
        ];

        for (timestamp, expected_date) in test_cases {
            let system_time = SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp);
            let result = system_time_to_datetime(Ok(system_time));

            assert!(result.is_some());
            let datetime = result.unwrap();
            assert_eq!(datetime.timestamp(), timestamp as i64);
            assert_eq!(datetime.format("%Y-%m-%d").to_string(), expected_date);
        }
    }

    #[test]
    fn test_system_time_to_datetime_leap_second() {
        // Test around a leap second (though Rust/chrono might not handle actual leap seconds)
        let leap_second_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_483_228_799);
        let result = system_time_to_datetime(Ok(leap_second_time));

        assert!(result.is_some());
        let datetime = result.unwrap();
        assert_eq!(datetime.format("%Y-%m-%d %H:%M:%S").to_string(), "2016-12-31 23:59:59");
    }

    #[test]
    fn test_system_time_to_datetime_subsec_precision() {
        // Test various subsecond precisions
        let test_cases = vec![
            (0, 0),                     // No subseconds
            (1, 1),                     // 1 nanosecond
            (1_000, 1_000),             // 1 microsecond
            (1_000_000, 1_000_000),     // 1 millisecond
            (999_999_999, 999_999_999), // Maximum nanoseconds
        ];

        for (input_nanos, expected_nanos) in test_cases {
            let system_time = SystemTime::UNIX_EPOCH + Duration::new(1_000_000, input_nanos);
            let result = system_time_to_datetime(Ok(system_time));

            assert!(result.is_some());
            let datetime = result.unwrap();
            assert_eq!(datetime.timestamp_subsec_nanos(), expected_nanos);
        }
    }

    #[test]
    fn test_system_time_to_datetime_io_error_kinds() {
        // Test various IO error kinds
        let error_kinds = vec![
            io::ErrorKind::NotFound,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::TimedOut,
            io::ErrorKind::UnexpectedEof,
        ];

        for kind in error_kinds {
            let io_error = io::Error::new(kind, "test error");
            let result = system_time_to_datetime(Err(io_error));
            assert!(result.is_none());
        }
    }
}
