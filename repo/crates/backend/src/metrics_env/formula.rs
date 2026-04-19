//! Pure formula implementations for metric computations.
//!
//! All functions are pure: they take a time-series window and parameters and
//! return an optional result (None when the window has insufficient data).
//!
//! # Supported formulas
//!
//! ## moving_average
//! Average of all values `v` where `ts` ∈ `[at − window_seconds, at]`.
//! Returns `None` if no points fall in the window.
//!
//! ## rate_of_change
//! `(v_last − v_first) / (ts_last − ts_first)` in value-per-second, computed
//! over the window `[at − window_seconds, at]`. Returns `None` when there are
//! fewer than two distinct timestamps in the window, or when the time span is
//! zero.
//!
//! ## comfort_index
//! Standard Effective Temperature formula for occupant comfort:
//!   `index = T - 0.55 × (1 - RH/100) × (T - 14.5)`
//! where `T` is temperature_c (latest value from sources[0]) and
//! `RH` is humidity_pct (latest value from sources[1]).
//! Result is clamped to `[-20.0, 50.0]`.
//!
//! Reference: Missenard (1937) comfort index widely used in building
//! management systems. The formula is self-documenting; no external library
//! is needed.
//!
//! The `params` JSONB field for `comfort_index` is intentionally empty `{}`
//! because the formula has no user-configurable parameters beyond the source
//! selection (temperature source at index 0, humidity source at index 1 in
//! the `source_ids` array of the definition).

use chrono::{DateTime, Utc};

/// A time-series window element.
pub type WindowPoint = (DateTime<Utc>, f64);

/// Compute the moving average of `window` values that fall in
/// `[at − window_seconds, at]`.
///
/// # Examples
/// ```
/// use chrono::Utc;
/// use terraops_backend::metrics_env::formula::moving_average;
/// let now = Utc::now();
/// let result = moving_average(&[], 3600, now);
/// assert!(result.is_none());
/// ```
pub fn moving_average(
    window: &[WindowPoint],
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<f64> {
    let cutoff = at - chrono::Duration::seconds(window_seconds);
    let values: Vec<f64> = window
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
        .map(|(_, v)| *v)
        .collect();
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// Compute the rate of change (value/second) over the window
/// `[at − window_seconds, at]`.
///
/// Returns `None` when:
///   * fewer than two points are in the window, or
///   * the time span between first and last is zero.
pub fn rate_of_change(
    window: &[WindowPoint],
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<f64> {
    let cutoff = at - chrono::Duration::seconds(window_seconds);
    let mut pts: Vec<(DateTime<Utc>, f64)> = window
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
        .cloned()
        .collect();
    if pts.len() < 2 {
        return None;
    }
    pts.sort_by_key(|(ts, _)| *ts);
    let (ts_first, v_first) = pts.first().unwrap();
    let (ts_last, v_last) = pts.last().unwrap();
    let dt_seconds = (*ts_last - *ts_first).num_milliseconds() as f64 / 1000.0;
    if dt_seconds == 0.0 {
        return None;
    }
    Some((v_last - v_first) / dt_seconds)
}

/// Compute the comfort index (Standard Effective Temperature approximation).
///
/// Expects `window` to contain two distinct sources interleaved:
///   * `temp_window` — temperature values in °C (from sources[0])
///   * `humidity_window` — relative humidity in % (from sources[1])
///
/// Uses the *latest* value from each source within the window.
///
/// Formula: `index = T - 0.55 × (1 - RH/100) × (T - 14.5)`
/// Clamped to `[-20.0, 50.0]`.
///
/// Returns `None` if either source has no values in the window.
pub fn comfort_index(
    temp_window: &[WindowPoint],
    humidity_window: &[WindowPoint],
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<f64> {
    let cutoff = at - chrono::Duration::seconds(window_seconds);

    let latest_temp = temp_window
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
        .max_by_key(|(ts, _)| *ts)
        .map(|(_, v)| *v)?;

    let latest_humidity = humidity_window
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
        .max_by_key(|(ts, _)| *ts)
        .map(|(_, v)| *v)?;

    let t = latest_temp;
    let rh = latest_humidity;
    let raw = t - 0.55 * (1.0 - rh / 100.0) * (t - 14.5);
    Some(raw.clamp(-20.0, 50.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs_from_epoch: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs_from_epoch, 0).unwrap()
    }

    // -----------------------------------------------------------------------
    // moving_average
    // -----------------------------------------------------------------------

    #[test]
    fn ma_empty_window_returns_none() {
        assert_eq!(moving_average(&[], 3600, ts(0)), None);
    }

    #[test]
    fn ma_single_point_in_window_returns_that_value() {
        let pts = vec![(ts(0), 42.0)];
        assert_eq!(moving_average(&pts, 3600, ts(100)), Some(42.0));
    }

    #[test]
    fn ma_point_outside_window_excluded() {
        // cutoff = at(3600) - window(3600) = 0; only pts at ts > 0 are included
        let pts = vec![(ts(-1), 10.0), (ts(100), 20.0), (ts(3600), 30.0)];
        // ts(-1) is before cutoff (0), excluded
        let result = moving_average(&pts, 3600, ts(3600));
        assert!(result.is_some());
        let avg = result.unwrap();
        // avg of 20.0 and 30.0 = 25.0
        assert!((avg - 25.0).abs() < 1e-9);
    }

    #[test]
    fn ma_multi_point_correct_average() {
        let pts = vec![(ts(0), 10.0), (ts(100), 20.0), (ts(200), 30.0)];
        let result = moving_average(&pts, 3600, ts(300));
        assert_eq!(result, Some(20.0));
    }

    #[test]
    fn ma_all_points_outside_window_none() {
        let pts = vec![(ts(0), 10.0)];
        // window ends at at=100, cutoff = 100 - 50 = 50; ts(0) < 50 excluded
        assert_eq!(moving_average(&pts, 50, ts(100)), None);
    }

    // -----------------------------------------------------------------------
    // rate_of_change
    // -----------------------------------------------------------------------

    #[test]
    fn roc_empty_window_none() {
        assert_eq!(rate_of_change(&[], 3600, ts(0)), None);
    }

    #[test]
    fn roc_single_point_none() {
        let pts = vec![(ts(0), 5.0)];
        assert_eq!(rate_of_change(&pts, 3600, ts(100)), None);
    }

    #[test]
    fn roc_two_points_correct() {
        // +10 value over 100 seconds = 0.1 /s
        let pts = vec![(ts(0), 0.0), (ts(100), 10.0)];
        let result = rate_of_change(&pts, 3600, ts(100));
        assert!(result.is_some());
        assert!((result.unwrap() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn roc_same_timestamp_none() {
        let pts = vec![(ts(50), 0.0), (ts(50), 10.0)];
        assert_eq!(rate_of_change(&pts, 3600, ts(100)), None);
    }

    #[test]
    fn roc_negative_rate() {
        // -20 over 100 seconds = -0.2 /s
        let pts = vec![(ts(0), 20.0), (ts(100), 0.0)];
        let result = rate_of_change(&pts, 3600, ts(100));
        assert!((result.unwrap() - (-0.2)).abs() < 1e-9);
    }

    // -----------------------------------------------------------------------
    // comfort_index
    // -----------------------------------------------------------------------

    #[test]
    fn ci_empty_temp_none() {
        let humidity = vec![(ts(0), 50.0)];
        assert_eq!(comfort_index(&[], &humidity, 3600, ts(100)), None);
    }

    #[test]
    fn ci_empty_humidity_none() {
        let temp = vec![(ts(0), 22.0)];
        assert_eq!(comfort_index(&temp, &[], 3600, ts(100)), None);
    }

    #[test]
    fn ci_standard_case() {
        // T=22°C, RH=60% → index = 22 - 0.55*(1-0.6)*(22-14.5)
        //                         = 22 - 0.55*0.4*7.5
        //                         = 22 - 1.65 = 20.35
        let temp = vec![(ts(0), 22.0)];
        let humidity = vec![(ts(0), 60.0)];
        let result = comfort_index(&temp, &humidity, 3600, ts(100));
        assert!(result.is_some());
        assert!((result.unwrap() - 20.35).abs() < 1e-9);
    }

    #[test]
    fn ci_clamp_upper() {
        // Extreme heat
        let temp = vec![(ts(0), 100.0)];
        let humidity = vec![(ts(0), 0.0)];
        let result = comfort_index(&temp, &humidity, 3600, ts(100)).unwrap();
        assert_eq!(result, 50.0);
    }

    #[test]
    fn ci_clamp_lower() {
        // Extreme cold
        let temp = vec![(ts(0), -100.0)];
        let humidity = vec![(ts(0), 100.0)];
        let result = comfort_index(&temp, &humidity, 3600, ts(100)).unwrap();
        assert_eq!(result, -20.0);
    }

    #[test]
    fn ci_uses_latest_value_in_window() {
        // Two temp readings; latest should win
        let temp = vec![(ts(0), 10.0), (ts(200), 22.0)];
        let humidity = vec![(ts(0), 60.0)];
        let result_a = comfort_index(&temp, &humidity, 3600, ts(300)).unwrap();
        // Should use T=22 (latest within window)
        let temp_b = vec![(ts(0), 22.0)];
        let result_b = comfort_index(&temp_b, &humidity, 3600, ts(300)).unwrap();
        assert!((result_a - result_b).abs() < 1e-9);
    }
}
