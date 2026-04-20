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
//! ## sku_on_shelf_compliance
//! Retail shelf-compliance percentage: the share of observations within the
//! window whose value is non-zero (i.e. the SKU was observed on-shelf).
//! Each contributing source represents one tracked SKU feed; the value of
//! each observation is typically 1.0 (on-shelf) or 0.0 (missing), so the
//! mean × 100 is the compliance %. Non-boolean inputs are tolerated: any
//! strictly-positive value counts as on-shelf. Returns `None` when the
//! window has no observations at all — the KPI reads as "no signal yet"
//! rather than 0 %.
//!
//! ## comfort_index
//! Occupant comfort index (extended Missenard with air-speed cooling):
//!
//!   base     = T - 0.55 × (1 - RH/100) × (T - 14.5)
//!   cooling  = 1.8 × sqrt(max(0, V - 0.1))   (for V > 0.1 m/s)
//!   index    = clamp(base - cooling, -20.0, 50.0)
//!
//! where:
//!   * `T`  — temperature °C (latest value from sources[0])
//!   * `RH` — relative humidity % (latest value from sources[1])
//!   * `V`  — air speed m/s (latest value from sources[2], optional)
//!
//! When no air-speed source is attached (definition uses only 2 sources),
//! the cooling term is zero and the output matches the base Missenard index.
//!
//! The computation also returns two quality dimensions:
//!   * `alignment`  — 0..1, how well the latest sample timestamps of the
//!                    contributing sources line up with the computation
//!                    instant `at`. 1.0 = all sources fresh, 0.0 = one or
//!                    more sources are a full window old. Captures
//!                    temporal drift between input sensors.
//!   * `confidence` — 0..1, combines source count (2 vs 3 sources present)
//!                    and sample density within the window. Lets dashboards
//!                    mark air-speed-missing comfort as "partial" without
//!                    throwing the value away.
//!
//! References: Missenard (1937); ASHRAE 55 simplified air-speed cooling.

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

/// Audit #13 Issue #2 — SKU on-shelf compliance percentage.
///
/// Returns `Some(pct)` where `pct ∈ [0.0, 100.0]` is the share of window
/// observations whose value is strictly greater than zero (on-shelf).
/// Returns `None` when the window contains no observations.
pub fn sku_on_shelf_compliance(
    window: &[WindowPoint],
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<f64> {
    let cutoff = at - chrono::Duration::seconds(window_seconds);
    let in_window: Vec<f64> = window
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
        .map(|(_, v)| *v)
        .collect();
    if in_window.is_empty() {
        return None;
    }
    let on_shelf = in_window.iter().filter(|v| **v > 0.0).count() as f64;
    Some((on_shelf / in_window.len() as f64) * 100.0)
}

/// Full output of the comfort-index computation, including the alignment
/// and confidence quality dimensions the audit contract requires.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComfortOutput {
    pub value: f64,
    /// 0..1 — how well the latest sample timestamps from each source line
    /// up with `at`. 1.0 = every source is fresh, 0.0 = at least one
    /// source is a full window old.
    pub alignment: f64,
    /// 0..1 — combines source count (2 vs 3) with sample density within
    /// the window. Tuned so dense three-source comfort scores close to
    /// 1.0, and partial two-source comfort scores ~0.6.
    pub confidence: f64,
}

/// Compute the comfort index (temperature + humidity + optional air speed).
///
/// Uses the *latest* value from each source within the window. Returns
/// `None` if either required source (temperature or humidity) has no
/// values in the window. Air speed is optional: when present, it
/// contributes a cooling term and increases the confidence score.
pub fn comfort_index_ext(
    temp_window: &[WindowPoint],
    humidity_window: &[WindowPoint],
    air_speed_window: Option<&[WindowPoint]>,
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<ComfortOutput> {
    let cutoff = at - chrono::Duration::seconds(window_seconds);

    let latest_in = |w: &[WindowPoint]| {
        w.iter()
            .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
            .max_by_key(|(ts, _)| *ts)
            .map(|(ts, v)| (*ts, *v))
    };

    let (temp_ts, t) = latest_in(temp_window)?;
    let (hum_ts, rh) = latest_in(humidity_window)?;

    // Extended Missenard base term.
    let base = t - 0.55 * (1.0 - rh / 100.0) * (t - 14.5);

    // Optional air-speed cooling term (ASHRAE-55 simplified).
    let (cooling, air_ts, has_air) = match air_speed_window.and_then(latest_in) {
        Some((ts, v)) => {
            let above = (v - 0.1).max(0.0);
            (1.8 * above.sqrt(), Some(ts), true)
        }
        None => (0.0, None, false),
    };

    let value = (base - cooling).clamp(-20.0, 50.0);

    // Alignment: derived from the staleness of the *least fresh* source.
    // offset_ratio = staleness / window_seconds, clamped to [0,1].
    let window_f = window_seconds.max(1) as f64;
    let staleness = |ts: DateTime<Utc>| {
        ((at - ts).num_milliseconds() as f64 / 1000.0 / window_f)
            .clamp(0.0, 1.0)
    };
    let mut worst_offset = staleness(temp_ts).max(staleness(hum_ts));
    if let Some(ts) = air_ts {
        worst_offset = worst_offset.max(staleness(ts));
    }
    let alignment = (1.0 - worst_offset).clamp(0.0, 1.0);

    // Confidence: source-count factor × sample-density factor.
    // Source count: 2 sources = 0.66, 3 sources = 1.0.
    let src_factor = if has_air { 1.0 } else { 2.0 / 3.0 };
    // Sample density: count samples across all sources, compared to a
    // tuned target. A single in-window sample per source already gives
    // full density credit (since comfort uses latest-in-window). Extra
    // samples do not penalize the score; fewer than one sample leaves
    // the source out entirely (None result above).
    let expected_per_source = 1.0_f64;
    let count_in = |w: &[WindowPoint]| {
        w.iter()
            .filter(|(ts, _)| *ts >= cutoff && *ts <= at)
            .count() as f64
    };
    let t_density = (count_in(temp_window) / expected_per_source).min(1.0);
    let h_density = (count_in(humidity_window) / expected_per_source).min(1.0);
    let a_density = air_speed_window
        .map(|w| (count_in(w) / expected_per_source).min(1.0))
        .unwrap_or(1.0); // not contributing when missing
    let density_factor = if has_air {
        (t_density + h_density + a_density) / 3.0
    } else {
        (t_density + h_density) / 2.0
    };
    let confidence = (src_factor * density_factor).clamp(0.0, 1.0);

    Some(ComfortOutput { value, alignment, confidence })
}

/// Backward-compatible two-source comfort index returning only the scalar
/// value. New code should prefer `comfort_index_ext` to also surface
/// alignment + confidence.
pub fn comfort_index(
    temp_window: &[WindowPoint],
    humidity_window: &[WindowPoint],
    window_seconds: i64,
    at: DateTime<Utc>,
) -> Option<f64> {
    comfort_index_ext(temp_window, humidity_window, None, window_seconds, at)
        .map(|o| o.value)
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
    fn ci_ext_two_source_matches_legacy_and_partial_confidence() {
        // T=22, RH=60, no air_speed → value=20.35, confidence reflects 2/3.
        let temp = vec![(ts(100), 22.0)];
        let humidity = vec![(ts(100), 60.0)];
        let out = comfort_index_ext(&temp, &humidity, None, 3600, ts(100)).unwrap();
        assert!((out.value - 20.35).abs() < 1e-9);
        // Perfect alignment (ts == at).
        assert!((out.alignment - 1.0).abs() < 1e-9);
        // src_factor = 2/3; density capped at 1.0 per source => confidence = 2/3 * (0.5+0.5)/... etc.
        assert!(out.confidence < 1.0);
        assert!(out.confidence > 0.0);
    }

    #[test]
    fn ci_ext_three_source_applies_air_cooling() {
        // T=25, RH=50, V=1.5 m/s → cooling ≈ 1.8 * sqrt(1.4) ≈ 2.13
        // base = 25 - 0.55*(0.5)*(10.5) = 25 - 2.8875 = 22.1125
        // value ≈ 22.1125 - 2.1295 ≈ 19.98
        let temp = vec![(ts(100), 25.0)];
        let humidity = vec![(ts(100), 50.0)];
        let air = vec![(ts(100), 1.5)];
        let out = comfort_index_ext(&temp, &humidity, Some(&air), 3600, ts(100)).unwrap();
        let expected_base = 25.0 - 0.55 * 0.5 * 10.5;
        let expected_cool = 1.8 * (1.4_f64).sqrt();
        assert!((out.value - (expected_base - expected_cool)).abs() < 1e-6);
        assert!(out.confidence > 0.6); // three-source should outperform two-source
    }

    #[test]
    fn ci_ext_alignment_drops_with_stale_source() {
        // temp is fresh (ts=100), humidity is half a window stale (ts=50).
        // window=100 → humidity staleness = 50/100 = 0.5 → alignment = 0.5.
        let temp = vec![(ts(100), 22.0)];
        let humidity = vec![(ts(50), 60.0)];
        let out = comfort_index_ext(&temp, &humidity, None, 100, ts(100)).unwrap();
        assert!((out.alignment - 0.5).abs() < 1e-9);
    }

    #[test]
    fn ci_ext_missing_required_source_returns_none() {
        let humidity = vec![(ts(100), 50.0)];
        assert!(comfort_index_ext(&[], &humidity, None, 3600, ts(100)).is_none());
    }

    #[test]
    fn ci_ext_air_cooling_zero_below_threshold() {
        // V=0.05 m/s → below 0.1 threshold → cooling term == 0
        let temp = vec![(ts(100), 22.0)];
        let humidity = vec![(ts(100), 60.0)];
        let air = vec![(ts(100), 0.05)];
        let out = comfort_index_ext(&temp, &humidity, Some(&air), 3600, ts(100)).unwrap();
        assert!((out.value - 20.35).abs() < 1e-9);
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
