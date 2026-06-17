use serde::{Deserialize, Serialize};

pub const CROSSFADE_TRIGGER_BUFFER_MS: u64 = 250;
const SHUFFLE_HISTORY_MAX: usize = 100;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "one" => Self::One,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

/// Stack of previously played queue indices used so Previous in shuffle mode
/// returns the actual last-played track rather than a random new one.
/// Intentionally `Mutex`-free — the app wraps this in state management as needed.
pub struct ShuffleHistory(Vec<i64>);

impl ShuffleHistory {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, index: i64) {
        self.0.push(index);
        if self.0.len() > SHUFFLE_HISTORY_MAX {
            self.0.remove(0);
        }
    }

    pub fn pop(&mut self) -> Option<i64> {
        self.0.pop()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl Default for ShuffleHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the next queue index given current position, queue length, and mode.
/// Returns `None` when the queue is exhausted (end of queue, no repeat).
pub fn next_index(
    current: i64,
    length: i64,
    shuffle: bool,
    repeat: &RepeatMode,
) -> Option<i64> {
    if length == 0 {
        return None;
    }
    match repeat {
        RepeatMode::One => Some(current),
        _ => {
            if shuffle && length > 1 {
                use std::time::SystemTime;
                let seed = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let mut pick = (seed as i64).rem_euclid(length);
                if pick == current {
                    pick = (pick + 1).rem_euclid(length);
                }
                Some(pick)
            } else {
                let next = current + 1;
                if next >= length {
                    if matches!(repeat, RepeatMode::All) {
                        Some(0)
                    } else {
                        None
                    }
                } else {
                    Some(next)
                }
            }
        }
    }
}

/// Returns true when the current track is close enough to its end that
/// a crossfade to the next track should begin.
/// `remaining_ms` is `duration_ms - position_ms`; `crossfade_ms` must be > 0.
pub fn should_crossfade(remaining_ms: u64, crossfade_ms: u64) -> bool {
    crossfade_ms > 0 && remaining_ms <= crossfade_ms + CROSSFADE_TRIGGER_BUFFER_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    // next_index tests

    #[test]
    fn empty_queue_returns_none() {
        assert_eq!(next_index(0, 0, false, &RepeatMode::Off), None);
        assert_eq!(next_index(0, 0, true, &RepeatMode::All), None);
    }

    #[test]
    fn sequential_advances() {
        assert_eq!(next_index(0, 3, false, &RepeatMode::Off), Some(1));
        assert_eq!(next_index(1, 3, false, &RepeatMode::Off), Some(2));
    }

    #[test]
    fn end_of_queue_no_repeat_returns_none() {
        assert_eq!(next_index(2, 3, false, &RepeatMode::Off), None);
    }

    #[test]
    fn repeat_all_wraps_to_zero() {
        assert_eq!(next_index(2, 3, false, &RepeatMode::All), Some(0));
    }

    #[test]
    fn repeat_one_stays() {
        assert_eq!(next_index(1, 3, false, &RepeatMode::One), Some(1));
        assert_eq!(next_index(0, 1, false, &RepeatMode::One), Some(0));
    }

    #[test]
    fn shuffle_never_returns_current_when_length_gt_1() {
        // Run many times to cover the seed-collision branch.
        for _ in 0..200 {
            let pick = next_index(2, 5, true, &RepeatMode::Off).unwrap();
            assert_ne!(pick, 2, "shuffle must not return the current index");
            assert!((0..5).contains(&pick));
        }
    }

    #[test]
    fn shuffle_with_single_track_returns_same() {
        // length == 1: shuffle branch is bypassed, sequential path runs; next == length → None (Off)
        assert_eq!(next_index(0, 1, true, &RepeatMode::Off), None);
        assert_eq!(next_index(0, 1, true, &RepeatMode::All), Some(0));
        assert_eq!(next_index(0, 1, true, &RepeatMode::One), Some(0));
    }

    // should_crossfade tests

    #[test]
    fn crossfade_zero_never_triggers() {
        assert!(!should_crossfade(100, 0));
        assert!(!should_crossfade(0, 0));
    }

    #[test]
    fn crossfade_triggers_within_window() {
        // exactly at crossfade_ms + buffer
        assert!(should_crossfade(5_000 + CROSSFADE_TRIGGER_BUFFER_MS, 5_000));
        assert!(should_crossfade(1_000, 5_000));
        assert!(should_crossfade(0, 5_000));
    }

    #[test]
    fn crossfade_does_not_trigger_outside_window() {
        assert!(!should_crossfade(5_000 + CROSSFADE_TRIGGER_BUFFER_MS + 1, 5_000));
        assert!(!should_crossfade(10_000, 5_000));
    }

    // ShuffleHistory tests

    #[test]
    fn history_push_pop_lifo() {
        let mut h = ShuffleHistory::new();
        h.push(0);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(0));
        assert_eq!(h.pop(), None);
    }

    #[test]
    fn history_caps_at_max() {
        let mut h = ShuffleHistory::new();
        for i in 0..=(SHUFFLE_HISTORY_MAX as i64) {
            h.push(i);
        }
        assert_eq!(h.0.len(), SHUFFLE_HISTORY_MAX);
        // The oldest entry (0) should have been evicted.
        assert_eq!(h.0[0], 1);
    }

    #[test]
    fn history_clear() {
        let mut h = ShuffleHistory::new();
        h.push(1);
        h.push(2);
        h.clear();
        assert_eq!(h.pop(), None);
    }

    // RepeatMode tests

    #[test]
    fn repeat_mode_roundtrip() {
        for (s, m) in [("off", RepeatMode::Off), ("all", RepeatMode::All), ("one", RepeatMode::One)] {
            assert_eq!(RepeatMode::parse(s), m);
            assert_eq!(m.as_str(), s);
        }
    }

    #[test]
    fn repeat_mode_parse_unknown_defaults_to_off() {
        assert_eq!(RepeatMode::parse("garbage"), RepeatMode::Off);
    }
}
