//! Minimal cron parser + "next fire time" evaluator.
//!
//! Audit #12 Issue #1 — report jobs store a `cron` string and the scheduler
//! is supposed to evaluate it automatically. This module parses the classic
//! 5-field cron expression and computes the next firing minute at or after
//! a given reference time.
//!
//! # Supported grammar (POSIX-subset cron)
//!
//!   `<minute> <hour> <day-of-month> <month> <day-of-week>`
//!
//! where each field is one of:
//!
//!   * `*`              — every value in the field's range
//!   * `N`              — a single fixed value
//!   * `A-B`            — inclusive range
//!   * `A,B,C`          — explicit list of values (also allowed: `A-B,C`)
//!   * `*/N`            — step every `N` values over the full field range
//!   * `A-B/N`          — step every `N` values within a range
//!
//! Day-of-week uses 0–6 with 0 = Sunday (the Unix convention). Both `0` and
//! `7` are accepted as Sunday.
//!
//! A cron fires on a minute where **all** of: minute, hour, month match,
//! AND **either** day-of-month **or** day-of-week matches when both are
//! restricted (classic Vixie-cron OR rule); when one of dom/dow is `*`
//! and the other is restricted, only the restricted one is consulted.
//!
//! Unsupported: `@reboot`/`@hourly`/`@daily` shortcuts, seconds-field
//! (6-field) cron, `L`/`W` symbols. Unrecognized input returns
//! `Err(String)`. Callers that handle `None` return from `next_after`
//! should treat it as "cron will not fire in the search horizon" — this
//! happens only for genuinely unreachable dom+dow+month combinations
//! (e.g. `* * 31 2 *`, which never matches).

use chrono::{DateTime, Datelike, Duration, Timelike, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronSchedule {
    minutes: Vec<u32>,      // 0..=59
    hours: Vec<u32>,        // 0..=23
    dom: Vec<u32>,          // 1..=31
    months: Vec<u32>,       // 1..=12
    dow: Vec<u32>,          // 0..=6 (0 = Sunday)
    dom_restricted: bool,
    dow_restricted: bool,
}

impl CronSchedule {
    /// Parse a 5-field POSIX-subset cron expression.
    pub fn parse(expr: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "cron expression must have 5 whitespace-separated fields (got {})",
                fields.len()
            ));
        }
        let minutes = parse_field(fields[0], 0, 59, false)?;
        let hours = parse_field(fields[1], 0, 23, false)?;
        let dom = parse_field(fields[2], 1, 31, false)?;
        let months = parse_field(fields[3], 1, 12, false)?;
        // dow: 0 and 7 both mean Sunday; normalize 7→0 after parsing.
        let mut dow = parse_field(fields[4], 0, 7, false)?;
        if dow.iter().any(|&v| v == 7) {
            for v in dow.iter_mut() {
                if *v == 7 {
                    *v = 0;
                }
            }
            dow.sort();
            dow.dedup();
        }
        let dom_restricted = fields[2] != "*";
        let dow_restricted = fields[4] != "*";
        Ok(CronSchedule {
            minutes,
            hours,
            dom,
            months,
            dow,
            dom_restricted,
            dow_restricted,
        })
    }

    /// Return the earliest minute-aligned `DateTime<Utc>` strictly greater
    /// than `from` that matches this schedule. Searches up to 400 days
    /// ahead; returns `None` if nothing matches in that window (which only
    /// happens for genuinely unreachable expressions such as
    /// `0 0 31 2 *`).
    pub fn next_after(&self, from: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start from the next whole minute strictly greater than `from`.
        let start = from
            .with_second(0)?
            .with_nanosecond(0)?
            .checked_add_signed(Duration::minutes(1))?;
        let horizon = start + Duration::days(400);
        let mut t = start;
        while t <= horizon {
            if self.matches(t) {
                return Some(t);
            }
            t += Duration::minutes(1);
        }
        None
    }

    fn matches(&self, t: DateTime<Utc>) -> bool {
        if !self.minutes.contains(&t.minute()) {
            return false;
        }
        if !self.hours.contains(&t.hour()) {
            return false;
        }
        if !self.months.contains(&t.month()) {
            return false;
        }
        let dom_hit = self.dom.contains(&t.day());
        // chrono's weekday: Monday=1..Sunday=7. Cron uses 0=Sunday..6=Saturday.
        let dow_cron: u32 = match t.weekday() {
            chrono::Weekday::Sun => 0,
            chrono::Weekday::Mon => 1,
            chrono::Weekday::Tue => 2,
            chrono::Weekday::Wed => 3,
            chrono::Weekday::Thu => 4,
            chrono::Weekday::Fri => 5,
            chrono::Weekday::Sat => 6,
        };
        let dow_hit = self.dow.contains(&dow_cron);
        match (self.dom_restricted, self.dow_restricted) {
            (true, true) => dom_hit || dow_hit, // classic Vixie-cron OR
            (true, false) => dom_hit,
            (false, true) => dow_hit,
            (false, false) => true,
        }
    }
}

fn parse_field(s: &str, min: u32, max: u32, _dow: bool) -> Result<Vec<u32>, String> {
    let mut out: Vec<u32> = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("empty cron field fragment in {s:?}"));
        }
        // Split off optional /step suffix.
        let (range_part, step) = match part.split_once('/') {
            Some((r, st)) => {
                let n: u32 = st
                    .parse()
                    .map_err(|_| format!("bad cron step {st:?} in {s:?}"))?;
                if n == 0 {
                    return Err(format!("cron step must be > 0 (got {st:?})"));
                }
                (r, n)
            }
            None => (part, 1),
        };
        let (a, b) = if range_part == "*" {
            (min, max)
        } else if let Some((lo, hi)) = range_part.split_once('-') {
            let lo: u32 = lo
                .parse()
                .map_err(|_| format!("bad cron range lo {lo:?} in {s:?}"))?;
            let hi: u32 = hi
                .parse()
                .map_err(|_| format!("bad cron range hi {hi:?} in {s:?}"))?;
            if lo > hi {
                return Err(format!("cron range {lo}-{hi} has lo > hi"));
            }
            (lo, hi)
        } else {
            let v: u32 = range_part
                .parse()
                .map_err(|_| format!("bad cron value {range_part:?} in {s:?}"))?;
            (v, v)
        };
        if a < min || b > max {
            return Err(format!(
                "cron field value {a}-{b} out of range {min}..={max} in {s:?}"
            ));
        }
        let mut v = a;
        while v <= b {
            out.push(v);
            v = v.saturating_add(step);
        }
    }
    out.sort();
    out.dedup();
    if out.is_empty() {
        return Err(format!("cron field {s:?} produced no values"));
    }
    Ok(out)
}

/// Small helper that parses and returns the next fire time in one call.
/// Returns `Err(_)` if the expression does not parse; returns `Ok(None)`
/// if it parses but has no match in the 400-day horizon.
pub fn next_fire_after(expr: &str, from: DateTime<Utc>) -> Result<Option<DateTime<Utc>>, String> {
    let sch = CronSchedule::parse(expr)?;
    Ok(sch.next_after(from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn parse_every_minute() {
        let s = CronSchedule::parse("* * * * *").unwrap();
        assert_eq!(s.minutes.len(), 60);
        assert_eq!(s.hours.len(), 24);
    }

    #[test]
    fn parse_rejects_bad_arity() {
        assert!(CronSchedule::parse("* * * *").is_err());
        assert!(CronSchedule::parse("* * * * * *").is_err());
    }

    #[test]
    fn parse_rejects_out_of_range() {
        assert!(CronSchedule::parse("60 * * * *").is_err());
        assert!(CronSchedule::parse("* 24 * * *").is_err());
        assert!(CronSchedule::parse("* * 32 * *").is_err());
        assert!(CronSchedule::parse("* * * 13 *").is_err());
        assert!(CronSchedule::parse("* * * * 8").is_err());
    }

    #[test]
    fn next_after_hourly() {
        // Top of every hour.
        let s = CronSchedule::parse("0 * * * *").unwrap();
        let got = s.next_after(t(2026, 4, 20, 12, 34)).unwrap();
        assert_eq!(got, t(2026, 4, 20, 13, 0));
    }

    #[test]
    fn next_after_step_15min() {
        let s = CronSchedule::parse("*/15 * * * *").unwrap();
        // Just after :00 → next is :15
        let got = s.next_after(t(2026, 4, 20, 12, 3)).unwrap();
        assert_eq!(got, t(2026, 4, 20, 12, 15));
        // At :45 → next is :00 of next hour
        let got = s.next_after(t(2026, 4, 20, 12, 45)).unwrap();
        assert_eq!(got, t(2026, 4, 20, 13, 0));
    }

    #[test]
    fn next_after_daily_0900() {
        let s = CronSchedule::parse("0 9 * * *").unwrap();
        // After 9:00 on the 20th → next is 9:00 on the 21st
        let got = s.next_after(t(2026, 4, 20, 9, 0)).unwrap();
        assert_eq!(got, t(2026, 4, 21, 9, 0));
        // Before 9:00 on the 20th → later that same day
        let got = s.next_after(t(2026, 4, 20, 8, 30)).unwrap();
        assert_eq!(got, t(2026, 4, 20, 9, 0));
    }

    #[test]
    fn next_after_weekday_only() {
        // Fridays at 17:00 (dow=5)
        let s = CronSchedule::parse("0 17 * * 5").unwrap();
        // 2026-04-20 is a Monday → next Friday is 2026-04-24 17:00
        let got = s.next_after(t(2026, 4, 20, 12, 0)).unwrap();
        assert_eq!(got, t(2026, 4, 24, 17, 0));
    }

    #[test]
    fn next_after_unreachable_returns_none() {
        // Feb 31 never exists.
        let s = CronSchedule::parse("0 0 31 2 *").unwrap();
        assert!(s.next_after(t(2026, 1, 1, 0, 0)).is_none());
    }

    #[test]
    fn sunday_accepts_both_0_and_7() {
        let a = CronSchedule::parse("0 12 * * 0").unwrap();
        let b = CronSchedule::parse("0 12 * * 7").unwrap();
        assert_eq!(a.dow, b.dow);
    }

    #[test]
    fn list_and_range_fields() {
        let s = CronSchedule::parse("0,30 9-17 * * 1-5").unwrap();
        // Mon 2026-04-20 at 08:00 → next is 09:00
        assert_eq!(
            s.next_after(t(2026, 4, 20, 8, 0)).unwrap(),
            t(2026, 4, 20, 9, 0)
        );
        // Mon 17:45 → Tuesday 09:00
        assert_eq!(
            s.next_after(t(2026, 4, 20, 17, 45)).unwrap(),
            t(2026, 4, 21, 9, 0)
        );
    }
}
