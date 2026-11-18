//! Formatting helpers — port of `src/lib/format.ts` plus small text utils.

/// `m:ss` / `h:mm:ss`; `—` when unknown.
pub fn duration(ms: Option<i64>) -> String {
    let Some(ms) = ms else { return "—".into() };
    let total = (ms.max(0) / 1000) as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn duration_u(ms: Option<u64>) -> String {
    duration(ms.map(|v| v as i64))
}

/// Sleep-timer countdown: `m:ss` or `h:mm:ss`.
pub fn countdown(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// First letters of the first two words, upper-cased.
pub fn initials(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// "1 track" / "3 tracks".
pub fn plural(n: i64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Relative time for "Last Played" columns: Just now / Nm ago / Nh ago /
/// Nd ago, else a short date.
pub fn relative_time(ts: Option<i64>) -> String {
    let Some(ts) = ts else { return "—".into() };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // Stored values may be seconds or milliseconds.
    let ts_s = if ts > 100_000_000_000 { ts / 1000 } else { ts };
    let diff = now - ts_s;
    if diff < 60 {
        "Just now".into()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 86_400 * 7 {
        format!("{}d ago", diff / 86_400)
    } else {
        short_date(ts_s)
    }
}

/// `Mon D, YYYY` from a Unix timestamp (UTC), like `toLocaleDateString`.
pub fn short_date(ts_s: i64) -> String {
    let days = ts_s.div_euclid(86_400);
    // Civil-from-days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{} {}, {}", MONTHS[(m - 1) as usize], d, y)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration(None), "—");
        assert_eq!(duration(Some(0)), "0:00");
        assert_eq!(duration(Some(61_000)), "1:01");
        assert_eq!(duration(Some(3_726_000)), "1:02:06");
    }

    #[test]
    fn initials_two_words() {
        assert_eq!(initials("daft punk live"), "DP");
        assert_eq!(initials("Björk"), "B");
    }

    #[test]
    fn dates() {
        assert_eq!(short_date(0), "Jan 1, 1970");
        assert_eq!(short_date(1_700_000_000), "Nov 14, 2023");
    }
}
