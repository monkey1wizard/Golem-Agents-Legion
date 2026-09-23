//! Time helpers shared across GAL crates.

use chrono::Utc;

/// Current UTC timestamp as ISO 8601 string.
pub fn now_timestamp() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Current UTC date as a compact eight-digit string.
pub fn utc_date_compact() -> String {
    Utc::now().format("%Y%m%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_timestamp_is_iso8601() {
        let ts = now_timestamp();
        assert!(ts.ends_with('Z'), "timestamp should end with Z: {ts}");
        assert!(ts.contains('T'), "timestamp should contain T: {ts}");
        assert_eq!(ts.len(), 20, "expected YYYY-MM-DDTHH:MM:SSZ length 20");
    }

    #[test]
    fn utc_date_compact_is_eight_digits() {
        let date = utc_date_compact();
        assert_eq!(date.len(), 8, "expected YYYYMMDD length 8");
        assert!(
            date.chars().all(|c| c.is_ascii_digit()),
            "date should contain only digits: {date}"
        );
    }
}
