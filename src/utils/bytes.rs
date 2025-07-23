#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::cast_sign_loss)]
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn test_format_bytes_bytes() {
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(2048), "2.00 KB");
        assert_eq!(format_bytes(1024 * 1023), "1023.00 KB");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 2), "2.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 10 + 1024 * 512), "10.50 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1023), "1023.00 MB");
    }

    #[test]
    fn test_format_bytes_gigabytes() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 5 + 1024 * 1024 * 512), "5.50 GB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 1023), "1023.00 GB");
    }

    #[test]
    fn test_format_bytes_terabytes() {
        assert_eq!(format_bytes(1024_u64.pow(4)), "1.00 TB");
        assert_eq!(format_bytes(1024_u64.pow(4) * 2), "2.00 TB");
        assert_eq!(format_bytes(1024_u64.pow(4) * 10), "10.00 TB");
        assert_eq!(format_bytes(1024_u64.pow(4) * 100), "100.00 TB");
    }

    #[test]
    fn test_format_bytes_large_terabytes() {
        // Test values larger than 1024 TB (should still show as TB)
        assert_eq!(format_bytes(1024_u64.pow(4) * 2048), "2048.00 TB");
        assert_eq!(format_bytes(1024_u64.pow(4) * 10000), "10000.00 TB");
    }

    #[test]
    fn test_format_bytes_edge_cases() {
        // Just below threshold
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024 * 1024 - 1), "1024.00 KB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 - 1), "1024.00 MB");

        // Exactly at threshold
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_format_bytes_precision() {
        // Test decimal precision
        assert_eq!(format_bytes(1024 + 51), "1.05 KB"); // 1.0498... rounds to 1.05
        assert_eq!(format_bytes(1024 + 102), "1.10 KB"); // 1.0996... rounds to 1.10
        assert_eq!(format_bytes(1024 * 1024 + 1024 * 256), "1.25 MB");
        assert_eq!(format_bytes(1024 * 1024 + 1024 * 768), "1.75 MB");
    }

    #[test]
    fn test_format_bytes_maximum_u64() {
        // Test with maximum u64 value
        assert_eq!(format_bytes(u64::MAX), "16777216.00 TB");
    }

    #[test]
    fn test_format_bytes_common_file_sizes() {
        // Common file sizes
        assert_eq!(format_bytes(1024 * 100), "100.00 KB"); // Small document
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.00 MB"); // Photo
        assert_eq!(format_bytes(1024 * 1024 * 700), "700.00 MB"); // CD size
        assert_eq!(format_bytes(1024 * 1024 * 4700), "4.59 GB"); // DVD size
        assert_eq!(format_bytes(1024_u64.pow(3) * 25), "25.00 GB"); // Blu-ray
    }

    #[test]
    fn test_format_bytes_rounding() {
        // Test rounding behavior
        assert_eq!(format_bytes(1024 + 5), "1.00 KB"); // 1.0048... rounds to 1.00
        assert_eq!(format_bytes(1024 + 6), "1.01 KB"); // 1.0058... rounds to 1.01
        assert_eq!(format_bytes(1024 * 1024 * 1024 + 1024 * 1024 * 5), "1.00 GB"); // 1.0048... rounds to 1.00
    }

    #[test]
    fn test_format_bytes_incremental() {
        // Test smooth transitions between units
        let test_values = vec![
            (1023, "1023 B"),
            (1024, "1.00 KB"),
            (1025, "1.00 KB"),
            (1024 * 1023, "1023.00 KB"),
            (1024 * 1024 - 1, "1024.00 KB"),
            (1024 * 1024, "1.00 MB"),
            (1024 * 1024 + 1, "1.00 MB"),
        ];

        for (bytes, expected) in test_values {
            assert_eq!(format_bytes(bytes), expected);
        }
    }
}
