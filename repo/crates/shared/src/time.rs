//! Centralized time helpers.
//!
//! - Storage: TZ-aware `DateTime<Utc>`.
//! - Display: `MM/DD/YYYY hh:mm AM/PM` in the user's local timezone.
//!
//! The backend persists UTC; per-user timezone offsets are applied at display
//! time. All human-facing timestamps MUST flow through this module.

use chrono::{DateTime, FixedOffset, Utc};

/// Format a UTC timestamp in the canonical display form using the caller-
/// supplied offset from UTC (seconds).
pub fn format_display(ts: DateTime<Utc>, offset_seconds: i32) -> String {
    let offset = FixedOffset::east_opt(offset_seconds).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap());
    ts.with_timezone(&offset).format("%m/%d/%Y %I:%M %p").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn us_format_12_hour() {
        let t = Utc.with_ymd_and_hms(2026, 4, 18, 21, 5, 0).unwrap();
        // America/New_York (EDT) is UTC-4 on this date.
        let s = format_display(t, -4 * 3600);
        assert_eq!(s, "04/18/2026 05:05 PM");
    }
}
