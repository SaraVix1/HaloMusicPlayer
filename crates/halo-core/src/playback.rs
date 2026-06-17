/// Returns true if the position qualifies as a "played" track for local stats:
/// at least 30 s elapsed, or at least 50% of the track finished.
pub fn is_played(position_ms: u64, duration_ms: Option<u64>) -> bool {
    if position_ms >= 30_000 {
        return true;
    }
    if let Some(d) = duration_ms {
        if d > 0 && position_ms * 2 >= d {
            return true;
        }
    }
    false
}

/// Four minutes in milliseconds — the absolute scrobble cap per Last.fm rules.
const SCROBBLE_CAP_MS: u64 = 4 * 60 * 1000;

/// Returns true if a track qualifies to be scrobbled to Last.fm at `position_ms`.
///
/// This is deliberately distinct from [`is_played`] (which governs local play
/// counts). Last.fm's rules are stricter: the track must be longer than 30 s,
/// and must have been played for **at least half its length, or 4 minutes,
/// whichever comes first**. A 10-minute track therefore scrobbles at 4 min, not
/// at 30 s.
///
/// When the duration is unknown we cannot apply the half-length rule, so we fall
/// back to the 4-minute cap alone.
pub fn should_scrobble(position_ms: u64, duration_ms: Option<u64>) -> bool {
    let threshold = match duration_ms {
        // Tracks under 30 s are never scrobbled.
        Some(d) if d < 30_000 => return false,
        Some(d) => (d / 2).min(SCROBBLE_CAP_MS),
        None => SCROBBLE_CAP_MS,
    };
    position_ms >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_seconds_counts_as_played() {
        assert!(is_played(30_000, Some(120_000)));
        assert!(is_played(30_001, Some(120_000)));
    }

    #[test]
    fn below_threshold_not_played() {
        assert!(!is_played(29_999, Some(120_000)));
        assert!(!is_played(0, Some(120_000)));
    }

    #[test]
    fn fifty_percent_counts_as_played() {
        // 60 s of a 120 s track
        assert!(is_played(60_000, Some(120_000)));
        // exactly half: 1 s of a 2 s track
        assert!(is_played(1_000, Some(2_000)));
    }

    #[test]
    fn just_under_fifty_percent_not_played() {
        // Must be below 30 s AND below 50% to not count as played.
        assert!(!is_played(29_999, Some(60_001)));
    }

    #[test]
    fn no_duration_uses_only_absolute_threshold() {
        assert!(!is_played(29_999, None));
        assert!(is_played(30_000, None));
    }

    #[test]
    fn zero_duration_track_not_played_below_30s() {
        // d == 0 guard: position * 2 >= 0 would always be true, so we guard d > 0.
        assert!(!is_played(0, Some(0)));
        assert!(is_played(30_000, Some(0))); // absolute threshold still fires
    }

    #[test]
    fn very_short_track_below_half_not_played() {
        // 100 ms of a 500 ms track (< 30 s and < 50%) → not played
        assert!(!is_played(100, Some(500)));
    }

    #[test]
    fn very_short_track_at_half_counts_as_played() {
        // 250 ms of a 500 ms track = exactly 50%; 50% threshold fires even though < 30 s
        assert!(is_played(250, Some(500)));
    }

    // ── should_scrobble ──────────────────────────────────────────────────────

    #[test]
    fn scrobble_short_track_never() {
        // Under 30 s: never scrobbled, even when finished.
        assert!(!should_scrobble(29_000, Some(29_000)));
        assert!(!should_scrobble(20_000, Some(25_000)));
    }

    #[test]
    fn scrobble_normal_track_at_half() {
        // 3-minute track scrobbles at half (90 s), not before.
        assert!(!should_scrobble(89_000, Some(180_000)));
        assert!(should_scrobble(90_000, Some(180_000)));
    }

    #[test]
    fn scrobble_long_track_caps_at_four_minutes() {
        // 10-minute track: half would be 5 min, but the 4-minute cap applies first.
        assert!(!should_scrobble(239_000, Some(600_000)));
        assert!(should_scrobble(240_000, Some(600_000)));
        // Crucially, it does NOT scrobble at 30 s like is_played would.
        assert!(!should_scrobble(30_000, Some(600_000)));
        assert!(is_played(30_000, Some(600_000)));
    }

    #[test]
    fn scrobble_unknown_duration_uses_four_minute_cap() {
        assert!(!should_scrobble(239_000, None));
        assert!(should_scrobble(240_000, None));
    }

    #[test]
    fn scrobble_boundary_thirty_second_track() {
        // Exactly 30 s long: not < 30 s, so half-rule (15 s) applies.
        assert!(should_scrobble(15_000, Some(30_000)));
        assert!(!should_scrobble(14_000, Some(30_000)));
    }
}
