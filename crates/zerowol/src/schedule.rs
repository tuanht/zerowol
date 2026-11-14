//! Schedules: strict time/date parsing and timezone resolution.

use crate::model::{LogEntry, Settings};

pub const LOG_CAP: usize = 500;

fn digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// Strict `HH:MM`, 24 h.
pub fn parse_time(s: &str) -> Option<jiff::civil::Time> {
    let (h, m) = s.split_once(':')?;
    if h.len() != 2 || m.len() != 2 || !digits(h) || !digits(m) {
        return None;
    }
    jiff::civil::Time::new(h.parse().ok()?, m.parse().ok()?, 0, 0).ok()
}

/// Accepted years. Far from jiff's timestamp limits (year 9999), so every
/// accepted date converts to an instant in every zone.
pub const YEARS: std::ops::RangeInclusive<i16> = 1970..=2999;

/// Strict `YYYY-MM-DD`, year within `YEARS`.
pub fn parse_date(s: &str) -> Option<jiff::civil::Date> {
    let mut it = s.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some()
        || y.len() != 4
        || m.len() != 2
        || d.len() != 2
        || !digits(y)
        || !digits(m)
        || !digits(d)
    {
        return None;
    }
    let y: i16 = y.parse().ok()?;
    if !YEARS.contains(&y) {
        return None;
    }
    jiff::civil::Date::new(y, m.parse().ok()?, d.parse().ok()?).ok()
}

/// IANA lookup in the bundled tz database (never the system zoneinfo, the
/// release image has none).
pub fn zone(name: &str) -> Option<jiff::tz::TimeZone> {
    jiff::tz::db().get(name).ok()
}

fn env_zone(tz_env: Option<&str>) -> Option<(String, jiff::tz::TimeZone)> {
    let n = tz_env?.trim();
    if n.is_empty() || n == "UTC" || n == "Etc/UTC" {
        return None;
    }
    zone(n).map(|z| (n.to_string(), z))
}

fn settings_zone(settings: &Settings) -> Option<(String, jiff::tz::TimeZone)> {
    let n = settings.timezone.as_deref()?;
    zone(n).map(|z| (n.to_string(), z))
}

/// `settings.timezone` if it resolves, else the `TZ` env var if it names a real
/// non-UTC IANA zone, else none.
pub fn effective_tz(
    settings: &Settings,
    tz_env: Option<&str>,
) -> Option<(String, jiff::tz::TimeZone)> {
    settings_zone(settings).or_else(|| env_zone(tz_env))
}

pub fn tz_source(settings: &Settings, tz_env: Option<&str>) -> Option<&'static str> {
    if settings_zone(settings).is_some() {
        Some("settings")
    } else if env_zone(tz_env).is_some() {
        Some("env")
    } else {
        None
    }
}

/// Append to the ring buffer, dropping the oldest entries beyond `LOG_CAP`.
pub fn push_log(log: &mut Vec<LogEntry>, e: LogEntry) {
    log.push(e);
    if log.len() > LOG_CAP {
        let extra = log.len() - LOG_CAP;
        log.drain(..extra);
    }
}

/// One occurrence of a rule on a local date.
#[derive(Clone, Debug, PartialEq)]
pub struct Occ {
    pub date: jiff::civil::Date,
    pub at: jiff::Timestamp,
    pub skipped: bool,
}

/// Wall time on a date to an instant. A wall time in a DST gap resolves to the
/// transition itself (the first valid instant after it); a repeated wall time
/// resolves to the earlier instant. `None` if the instant is outside jiff's
/// timestamp range (only possible near year 9999).
pub fn instant(
    tz: &jiff::tz::TimeZone,
    date: jiff::civil::Date,
    time: jiff::civil::Time,
) -> Option<jiff::Timestamp> {
    use jiff::tz::AmbiguousOffset;
    let dt = date.to_datetime(time);
    let amb = tz.to_ambiguous_timestamp(dt);
    match amb.offset() {
        AmbiguousOffset::Unambiguous { offset } => offset.to_timestamp(dt).ok(),
        // Both offsets are valid for a fold; `before` is the earlier instant.
        AmbiguousOffset::Fold { before, .. } => before.to_timestamp(dt).ok(),
        AmbiguousOffset::Gap { after, .. } => {
            // The `after` offset is larger, so it yields an instant just before
            // the transition; the next transition is the one we want.
            let ts = after.to_timestamp(dt).ok()?;
            Some(tz.following(ts).next().map_or(ts, |t| t.timestamp()))
        }
    }
}

fn weekday_of(d: jiff::civil::Date) -> crate::model::Weekday {
    use crate::model::Weekday as W;
    use jiff::civil::Weekday as J;
    match d.weekday() {
        J::Monday => W::Mon,
        J::Tuesday => W::Tue,
        J::Wednesday => W::Wed,
        J::Thursday => W::Thu,
        J::Friday => W::Fri,
        J::Saturday => W::Sat,
        J::Sunday => W::Sun,
    }
}

/// All occurrences of `s` on local dates in `[from, to]` (inclusive), ignoring
/// `enabled`. An unparseable time or once-date yields none, and so does a date
/// whose instant is out of range.
pub fn expand(
    s: &crate::model::Schedule,
    tz: &jiff::tz::TimeZone,
    from: jiff::civil::Date,
    to: jiff::civil::Date,
) -> Vec<Occ> {
    use crate::model::{Repeat, Weekday as W};
    let Some(time) = parse_time(&s.time) else {
        return Vec::new();
    };
    let skips: Vec<jiff::civil::Date> = s.skip.iter().filter_map(|d| parse_date(d)).collect();
    let mut out = Vec::new();
    let mut d = from;
    while d <= to {
        let wd = weekday_of(d);
        let hit = match &s.repeat {
            Repeat::Once { date } => parse_date(date) == Some(d),
            Repeat::Daily => true,
            Repeat::Weekdays => !matches!(wd, W::Sat | W::Sun),
            Repeat::Weekends => matches!(wd, W::Sat | W::Sun),
            Repeat::Weekly { days } => days.contains(&wd),
        };
        if let Some(at) = hit.then(|| instant(tz, d, time)).flatten() {
            out.push(Occ {
                date: d,
                at,
                skipped: skips.contains(&d),
            });
        }
        match d.tomorrow() {
            Ok(n) => d = n,
            Err(_) => break,
        }
    }
    out
}

/// An occurrence can still fire up to this long after its time.
pub const GRACE_SECS: i64 = 120;
/// How far back the startup pass looks for missed occurrences.
pub const LOOKBACK_SECS: i64 = 7 * 86400;

/// An occurrence that is due (to fire, or to be logged missed).
#[derive(Clone, Debug, PartialEq)]
pub struct Due {
    pub schedule_id: String,
    pub machine_id: String,
    pub action: crate::model::SchedAction,
    pub date: String,
    pub at: jiff::Timestamp,
}

/// Enabled, non-skipped occurrences with `after < at <= now` and no log entry
/// for (id, date). Split into (fire, missed) by `now - at <= GRACE_SECS`.
/// Both lists are sorted by `at`.
pub fn select(
    state: &crate::model::State,
    tz: &jiff::tz::TimeZone,
    after: jiff::Timestamp,
    now: jiff::Timestamp,
) -> (Vec<Due>, Vec<Due>) {
    let (mut fire, mut missed) = (Vec::new(), Vec::new());
    if after >= now {
        return (fire, missed);
    }
    let from = after.to_zoned(tz.clone()).date();
    let to = now.to_zoned(tz.clone()).date();
    for s in state.schedules.iter().filter(|s| s.enabled) {
        for o in expand(s, tz, from, to) {
            if o.skipped || o.at <= after || o.at > now {
                continue;
            }
            let date = o.date.to_string();
            if state
                .schedule_log
                .iter()
                .any(|e| e.schedule_id == s.id && e.date == date)
            {
                continue;
            }
            let due = Due {
                schedule_id: s.id.clone(),
                machine_id: s.machine_id.clone(),
                action: s.action.clone(),
                date,
                at: o.at,
            };
            if now.as_second() - o.at.as_second() <= GRACE_SECS {
                fire.push(due);
            } else {
                missed.push(due);
            }
        }
    }
    fire.sort_by(|a, b| (a.at, &a.schedule_id).cmp(&(b.at, &b.schedule_id)));
    missed.sort_by(|a, b| (a.at, &a.schedule_id).cmp(&(b.at, &b.schedule_id)));
    (fire, missed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LogResult;

    fn st(tz: Option<&str>) -> Settings {
        Settings {
            timezone: tz.map(String::from),
        }
    }

    #[test]
    fn parse_time_strict() {
        let t = parse_time("07:30").unwrap();
        assert_eq!((t.hour(), t.minute()), (7, 30));
        for bad in ["7:30", "24:00", "07:60", "07:30:00", "", "+7:30", "07-30"] {
            assert!(parse_time(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn parse_date_strict() {
        let d = parse_date("2026-11-13").unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2026, 11, 13));
        for bad in [
            "2026-02-30",
            "13/11/2026",
            "2026-1-3",
            "2026-11-13T00:00",
            "",
        ] {
            assert!(parse_date(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn parse_date_bounded_years() {
        for bad in ["9999-12-31", "0000-01-01", "1969-12-31", "3000-01-01"] {
            assert!(parse_date(bad).is_none(), "{bad}");
        }
        for ok in ["1970-01-01", "2999-12-31"] {
            assert!(parse_date(ok).is_some(), "{ok}");
        }
    }

    #[test]
    fn instant_and_expand_never_panic_at_timestamp_max() {
        use crate::model::{Repeat, Schedule};
        let la = zone("America/Los_Angeles").unwrap();
        let end = jiff::civil::date(9999, 12, 31);
        let t = parse_time("23:59").unwrap();
        assert!(instant(&la, end, t).is_none());
        assert!(instant(&jiff::tz::TimeZone::UTC, end, t).is_none());
        let s = Schedule {
            id: "s".into(),
            machine_id: "m".into(),
            action: crate::model::SchedAction::Wake,
            time: "23:59".into(),
            repeat: Repeat::Daily,
            enabled: true,
            skip: Vec::new(),
            created: 0,
        };
        // Occurrences that can't be represented as instants are dropped.
        let got = expand(&s, &la, jiff::civil::date(9999, 12, 28), end);
        assert!(got.iter().all(|o| o.date < jiff::civil::date(9999, 12, 30)));
        assert!(!got.is_empty());
    }

    #[test]
    fn effective_tz_prefers_settings() {
        let (n, _) = effective_tz(&st(Some("Asia/Ho_Chi_Minh")), Some("Europe/Berlin")).unwrap();
        assert_eq!(n, "Asia/Ho_Chi_Minh");
        assert_eq!(
            tz_source(&st(Some("Asia/Ho_Chi_Minh")), Some("Europe/Berlin")),
            Some("settings")
        );
    }

    #[test]
    fn effective_tz_env_fallback() {
        let (n, _) = effective_tz(&st(None), Some("Europe/Berlin")).unwrap();
        assert_eq!(n, "Europe/Berlin");
        assert_eq!(tz_source(&st(None), Some("Europe/Berlin")), Some("env"));
        // a settings name that fails to resolve falls through to env
        assert_eq!(
            tz_source(&st(Some("Mars/Base")), Some("Europe/Berlin")),
            Some("env")
        );
    }

    #[test]
    fn effective_tz_ignores_utc_env() {
        for e in ["UTC", "Etc/UTC", ""] {
            assert!(effective_tz(&st(None), Some(e)).is_none(), "{e}");
            assert_eq!(tz_source(&st(None), Some(e)), None, "{e}");
        }
        assert!(effective_tz(&st(None), None).is_none());
    }

    #[test]
    fn effective_tz_bad_env() {
        assert!(effective_tz(&st(None), Some("Mars/Base")).is_none());
        assert_eq!(tz_source(&st(None), Some("Mars/Base")), None);
    }

    #[test]
    fn zone_resolves_from_bundled_db() {
        let z = zone("Europe/Berlin").unwrap();
        let ts = "2027-03-28T12:00"
            .parse::<jiff::civil::DateTime>()
            .unwrap()
            .to_zoned(z)
            .unwrap()
            .timestamp();
        assert_eq!(ts.to_string(), "2027-03-28T10:00:00Z");
    }

    #[test]
    fn push_log_caps() {
        let mut log = Vec::new();
        for i in 0..=LOG_CAP as u64 {
            push_log(
                &mut log,
                LogEntry {
                    schedule_id: "s".into(),
                    date: "2026-11-13".into(),
                    at: i,
                    result: LogResult::Ok,
                    detail: None,
                },
            );
        }
        assert_eq!(log.len(), LOG_CAP);
        assert_eq!(log[0].at, 1);
        assert_eq!(log.last().unwrap().at, LOG_CAP as u64);
    }

    fn sch(time: &str, repeat: crate::model::Repeat, skip: &[&str]) -> crate::model::Schedule {
        crate::model::Schedule {
            id: "s".into(),
            machine_id: "m".into(),
            action: crate::model::SchedAction::Wake,
            time: time.into(),
            repeat,
            enabled: true,
            skip: skip.iter().map(|s| s.to_string()).collect(),
            created: 0,
        }
    }
    fn d(s: &str) -> jiff::civil::Date {
        parse_date(s).unwrap()
    }
    fn ts(s: &str) -> jiff::Timestamp {
        s.parse().unwrap()
    }
    fn week(s: &crate::model::Schedule, tz: &str) -> Vec<Occ> {
        expand(s, &zone(tz).unwrap(), d("2026-11-09"), d("2026-11-15"))
    }

    #[test]
    fn daily_over_week() {
        use crate::model::Repeat;
        let o = week(&sch("08:00", Repeat::Daily, &[]), "Asia/Ho_Chi_Minh");
        assert_eq!(o.len(), 7);
        assert_eq!(o[0].at, ts("2026-11-09T01:00:00Z"));
    }

    #[test]
    fn weekdays_and_weekends() {
        use crate::model::Repeat;
        assert_eq!(
            week(&sch("08:00", Repeat::Weekdays, &[]), "Asia/Ho_Chi_Minh").len(),
            5
        );
        let e = week(&sch("08:00", Repeat::Weekends, &[]), "Asia/Ho_Chi_Minh");
        assert_eq!(
            e.iter().map(|o| o.date).collect::<Vec<_>>(),
            [d("2026-11-14"), d("2026-11-15")]
        );
    }

    #[test]
    fn weekly_days() {
        use crate::model::{Repeat, Weekday};
        let r = Repeat::Weekly {
            days: vec![Weekday::Tue, Weekday::Thu],
        };
        let o = week(&sch("08:00", r, &[]), "Asia/Ho_Chi_Minh");
        assert_eq!(
            o.iter().map(|o| o.date).collect::<Vec<_>>(),
            [d("2026-11-10"), d("2026-11-12")]
        );
    }

    #[test]
    fn once_in_and_out_of_range() {
        use crate::model::Repeat;
        let s = sch(
            "08:00",
            Repeat::Once {
                date: "2026-11-12".into(),
            },
            &[],
        );
        let tz = zone("Asia/Ho_Chi_Minh").unwrap();
        assert_eq!(expand(&s, &tz, d("2026-11-09"), d("2026-11-15")).len(), 1);
        assert_eq!(expand(&s, &tz, d("2026-11-13"), d("2026-11-15")).len(), 0);
    }

    #[test]
    fn skip_marks_occurrence() {
        use crate::model::Repeat;
        let o = week(
            &sch("08:00", Repeat::Daily, &["2026-11-11"]),
            "Asia/Ho_Chi_Minh",
        );
        for x in &o {
            assert_eq!(x.skipped, x.date == d("2026-11-11"));
        }
    }

    #[test]
    fn bad_time_yields_none() {
        use crate::model::Repeat;
        assert!(week(&sch("8:00", Repeat::Daily, &[]), "Europe/Berlin").is_empty());
    }

    #[test]
    fn dst_gap_berlin() {
        use crate::model::Repeat;
        let tz = zone("Europe/Berlin").unwrap();
        let o = expand(
            &sch("02:30", Repeat::Daily, &[]),
            &tz,
            d("2027-03-28"),
            d("2027-03-28"),
        );
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].at, ts("2027-03-28T01:00:00Z"));
    }

    #[test]
    fn dst_fold_berlin() {
        use crate::model::Repeat;
        let tz = zone("Europe/Berlin").unwrap();
        let o = expand(
            &sch("02:30", Repeat::Daily, &[]),
            &tz,
            d("2027-10-31"),
            d("2027-10-31"),
        );
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].at, ts("2027-10-31T00:30:00Z"));
    }

    #[test]
    fn range_across_dst_keeps_wall_time() {
        use crate::model::Repeat;
        let tz = zone("Europe/Berlin").unwrap();
        let o = expand(
            &sch("07:30", Repeat::Daily, &[]),
            &tz,
            d("2027-03-01"),
            d("2027-03-31"),
        );
        assert_eq!(o.len(), 31);
        for x in &o {
            let z = x.at.to_zoned(tz.clone());
            assert_eq!((z.date(), z.hour(), z.minute()), (x.date, 7, 30));
        }
    }

    #[test]
    fn tz_change_keeps_wall_time() {
        use crate::model::Repeat;
        let s = sch("07:30", Repeat::Daily, &[]);
        let a = week(&s, "Asia/Ho_Chi_Minh");
        let b = week(&s, "Europe/Berlin");
        for (o, name) in [(&a, "Asia/Ho_Chi_Minh"), (&b, "Europe/Berlin")] {
            let tz = zone(name).unwrap();
            for x in o {
                let z = x.at.to_zoned(tz.clone());
                assert_eq!((z.hour(), z.minute()), (7, 30));
            }
        }
        assert_ne!(a[0].at, b[0].at);
    }

    fn sel_state(s: crate::model::Schedule, log: Vec<LogEntry>) -> crate::model::State {
        crate::model::State {
            schedules: vec![s],
            schedule_log: log,
            ..Default::default()
        }
    }
    fn local(tz: &str, s: &str) -> jiff::Timestamp {
        s.parse::<jiff::civil::DateTime>()
            .unwrap()
            .to_zoned(zone(tz).unwrap())
            .unwrap()
            .timestamp()
    }
    fn entry(date: &str, result: LogResult) -> LogEntry {
        LogEntry {
            schedule_id: "s".into(),
            date: date.into(),
            at: 0,
            result,
            detail: None,
        }
    }
    const HCM: &str = "Asia/Ho_Chi_Minh";

    #[test]
    fn select_fires_within_grace() {
        use crate::model::Repeat;
        let st = sel_state(sch("07:30", Repeat::Daily, &[]), vec![]);
        let (fire, missed) = select(
            &st,
            &zone(HCM).unwrap(),
            local(HCM, "2026-11-14T07:29:50"),
            local(HCM, "2026-11-14T07:30:10"),
        );
        assert_eq!((fire.len(), missed.len()), (1, 0));
        let f = &fire[0];
        assert_eq!(
            (
                f.schedule_id.as_str(),
                f.machine_id.as_str(),
                f.date.as_str()
            ),
            ("s", "m", "2026-11-14")
        );
        assert_eq!(f.action, crate::model::SchedAction::Wake);
        assert_eq!(f.at, local(HCM, "2026-11-14T07:30:00"));
    }

    #[test]
    fn select_missed_beyond_grace() {
        use crate::model::Repeat;
        let st = sel_state(sch("07:30", Repeat::Daily, &[]), vec![]);
        let (fire, missed) = select(
            &st,
            &zone(HCM).unwrap(),
            local(HCM, "2026-11-14T07:00:00"),
            local(HCM, "2026-11-14T07:33:00"),
        );
        assert_eq!((fire.len(), missed.len()), (0, 1));
        assert_eq!(missed[0].date, "2026-11-14");
    }

    #[test]
    fn select_window_is_half_open() {
        use crate::model::Repeat;
        let st = sel_state(sch("07:30", Repeat::Daily, &[]), vec![]);
        let tz = zone(HCM).unwrap();
        let at = local(HCM, "2026-11-14T07:30:00");
        let (f, m) = select(&st, &tz, at, local(HCM, "2026-11-14T07:30:15"));
        assert_eq!((f.len(), m.len()), (0, 0), "after == at excluded");
        let (f, m) = select(&st, &tz, local(HCM, "2026-11-14T07:29:45"), at);
        assert_eq!((f.len(), m.len()), (1, 0), "now == at included");
    }

    #[test]
    fn select_skips_logged_dates() {
        use crate::model::Repeat;
        let st = sel_state(
            sch("07:30", Repeat::Daily, &[]),
            vec![entry("2026-11-14", LogResult::Firing)],
        );
        let (fire, missed) = select(
            &st,
            &zone(HCM).unwrap(),
            local(HCM, "2026-11-14T07:00:00"),
            local(HCM, "2026-11-14T07:30:10"),
        );
        assert_eq!((fire.len(), missed.len()), (0, 0));
    }

    #[test]
    fn select_ignores_disabled_and_skipped() {
        use crate::model::Repeat;
        let tz = zone(HCM).unwrap();
        let (a, n) = (
            local(HCM, "2026-11-14T07:29:50"),
            local(HCM, "2026-11-14T07:30:10"),
        );
        let mut s = sch("07:30", Repeat::Daily, &[]);
        s.enabled = false;
        let (f, m) = select(&sel_state(s, vec![]), &tz, a, n);
        assert_eq!((f.len(), m.len()), (0, 0));
        let s = sch("07:30", Repeat::Daily, &["2026-11-14"]);
        let (f, m) = select(&sel_state(s, vec![]), &tz, a, n);
        assert_eq!((f.len(), m.len()), (0, 0));
    }

    #[test]
    fn select_spans_dates() {
        use crate::model::Repeat;
        let st = sel_state(sch("07:30", Repeat::Daily, &[]), vec![]);
        let (fire, missed) = select(
            &st,
            &zone(HCM).unwrap(),
            local(HCM, "2026-11-12T06:00:00"),
            local(HCM, "2026-11-14T07:30:30"),
        );
        assert_eq!(fire.len(), 1);
        assert_eq!(
            missed.iter().map(|d| d.date.as_str()).collect::<Vec<_>>(),
            ["2026-11-12", "2026-11-13"]
        );
    }

    #[test]
    fn logged_date_never_refires_after_tz_change() {
        use crate::model::Repeat;
        // Fired at 07:30 Ho Chi Minh (00:30Z); zone then changed to Berlin,
        // whose 07:30 on the same date (06:30Z) is still ahead.
        let st = sel_state(
            sch("07:30", Repeat::Daily, &[]),
            vec![entry("2026-11-14", LogResult::Ok)],
        );
        let (fire, missed) = select(
            &st,
            &zone("Europe/Berlin").unwrap(),
            local("Europe/Berlin", "2026-11-14T07:29:00"),
            local("Europe/Berlin", "2026-11-14T07:30:30"),
        );
        assert_eq!((fire.len(), missed.len()), (0, 0));
    }
}
