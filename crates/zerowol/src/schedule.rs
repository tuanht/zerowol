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

/// Strict `YYYY-MM-DD`.
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
    jiff::civil::Date::new(y.parse().ok()?, m.parse().ok()?, d.parse().ok()?).ok()
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
}
