use serde::{Deserialize, Serialize};

pub const CROSSFADE_TRIGGER_BUFFER_MS: u64 = 250;

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

/// Compute the next queue index in **sequential** (non-shuffle) mode.
/// Returns `None` when the queue is exhausted (end of queue, no repeat).
/// Shuffle mode uses [`ShuffleOrder`] instead.
pub fn next_index(current: i64, length: i64, repeat: &RepeatMode) -> Option<i64> {
    if length == 0 {
        return None;
    }
    match repeat {
        RepeatMode::One => Some(current),
        _ => {
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

/// The shuffled play order for the queue: a permutation of queue indices
/// (generated with Fisher-Yates) plus a cursor. Next walks forward through
/// `order`, Previous walks back, so every track plays exactly once per pass and
/// Previous always retraces what Next played.
///
/// Persisted by the app as JSON; `is_consistent` lets the caller detect a
/// stale order (queue changed underneath it) and regenerate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShuffleOrder {
    /// Queue indices in play order.
    pub order: Vec<i64>,
    /// Cursor into `order` — the currently playing entry.
    pub pos: usize,
}

impl ShuffleOrder {
    /// Fresh shuffled order over `0..len`, with `start` first (it's the track
    /// that is playing / about to play).
    pub fn new(len: i64, start: i64, rng: &mut fastrand::Rng) -> Self {
        let mut order: Vec<i64> = (0..len.max(0)).collect();
        fisher_yates(&mut order, rng);
        if let Some(i) = order.iter().position(|&x| x == start) {
            order.swap(0, i);
        }
        Self { order, pos: 0 }
    }

    /// The queue index currently playing, if any.
    pub fn current(&self) -> Option<i64> {
        self.order.get(self.pos).copied()
    }

    /// Move to the next track. Repeat One stays put; at the end of the order
    /// Repeat All reshuffles (never starting with the track just played, when
    /// there is a choice) and Repeat Off returns `None`.
    pub fn advance(&mut self, repeat: &RepeatMode, rng: &mut fastrand::Rng) -> Option<i64> {
        if self.order.is_empty() {
            return None;
        }
        if matches!(repeat, RepeatMode::One) {
            return self.current();
        }
        if self.pos + 1 < self.order.len() {
            self.pos += 1;
            return self.current();
        }
        if !matches!(repeat, RepeatMode::All) {
            return None;
        }
        let last = self.current();
        let len = self.order.len();
        fisher_yates(&mut self.order, rng);
        if len > 1 && self.order.first().copied() == last {
            let j = rng.usize(1..len);
            self.order.swap(0, j);
        }
        self.pos = 0;
        self.current()
    }

    /// Step back to the previously played track; `None` at the start.
    pub fn prev(&mut self) -> Option<i64> {
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        self.current()
    }

    /// The user explicitly picked queue index `idx`. Move it to play next and
    /// advance onto it, so the rest of the unplayed order is preserved — no
    /// track is skipped or repeated because of the jump.
    pub fn jump_to(&mut self, idx: i64) {
        let Some(i) = self.order.iter().position(|&x| x == idx) else {
            return;
        };
        if i == self.pos {
            return;
        }
        self.order.remove(i);
        if i < self.pos {
            self.pos -= 1;
        }
        self.order.insert(self.pos + 1, idx);
        self.pos += 1;
    }

    /// A track was inserted into the queue at index `idx` (later indices shift
    /// up). It is placed at a random point among the not-yet-played tracks.
    pub fn on_insert(&mut self, idx: i64, rng: &mut fastrand::Rng) {
        for x in self.order.iter_mut() {
            if *x >= idx {
                *x += 1;
            }
        }
        let lo = if self.order.is_empty() { 0 } else { self.pos + 1 };
        let at = rng.usize(lo..=self.order.len());
        self.order.insert(at, idx);
    }

    /// The track at queue index `idx` was removed (later indices shift down).
    /// If it was the current track, the cursor lands on the next one in the
    /// order (or the last one, if it was at the end).
    pub fn on_remove(&mut self, idx: i64) {
        let Some(i) = self.order.iter().position(|&x| x == idx) else {
            return;
        };
        self.order.remove(i);
        for x in self.order.iter_mut() {
            if *x > idx {
                *x -= 1;
            }
        }
        if i < self.pos {
            self.pos -= 1;
        }
        if self.pos >= self.order.len() {
            self.pos = self.order.len().saturating_sub(1);
        }
    }

    /// True if this is a valid order for a queue of `len` tracks: a
    /// permutation of `0..len` with the cursor in range.
    pub fn is_consistent(&self, len: i64) -> bool {
        if self.order.len() as i64 != len {
            return false;
        }
        if len > 0 && self.pos >= self.order.len() {
            return false;
        }
        let mut seen = vec![false; self.order.len()];
        self.order.iter().all(|&x| {
            let ok = x >= 0 && (x as usize) < seen.len() && !seen[x as usize];
            if ok {
                seen[x as usize] = true;
            }
            ok
        })
    }
}

/// In-place Fisher-Yates shuffle.
fn fisher_yates(v: &mut [i64], rng: &mut fastrand::Rng) {
    for i in (1..v.len()).rev() {
        let j = rng.usize(..=i);
        v.swap(i, j);
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

    fn rng(seed: u64) -> fastrand::Rng {
        fastrand::Rng::with_seed(seed)
    }

    // next_index tests

    #[test]
    fn empty_queue_returns_none() {
        assert_eq!(next_index(0, 0, &RepeatMode::Off), None);
        assert_eq!(next_index(0, 0, &RepeatMode::All), None);
    }

    #[test]
    fn sequential_advances() {
        assert_eq!(next_index(0, 3, &RepeatMode::Off), Some(1));
        assert_eq!(next_index(1, 3, &RepeatMode::Off), Some(2));
    }

    #[test]
    fn end_of_queue_no_repeat_returns_none() {
        assert_eq!(next_index(2, 3, &RepeatMode::Off), None);
    }

    #[test]
    fn repeat_all_wraps_to_zero() {
        assert_eq!(next_index(2, 3, &RepeatMode::All), Some(0));
    }

    #[test]
    fn repeat_one_stays() {
        assert_eq!(next_index(1, 3, &RepeatMode::One), Some(1));
        assert_eq!(next_index(0, 1, &RepeatMode::One), Some(0));
    }

    // ShuffleOrder tests

    #[test]
    fn shuffle_new_is_permutation_with_start_first() {
        for seed in 0..50 {
            let o = ShuffleOrder::new(10, 7, &mut rng(seed));
            assert!(o.is_consistent(10));
            assert_eq!(o.current(), Some(7));
        }
    }

    #[test]
    fn shuffle_plays_every_track_exactly_once() {
        for seed in 0..50 {
            let mut r = rng(seed);
            let mut o = ShuffleOrder::new(20, 3, &mut r);
            let mut played = vec![o.current().unwrap()];
            while let Some(i) = o.advance(&RepeatMode::Off, &mut r) {
                played.push(i);
            }
            played.sort();
            assert_eq!(played, (0..20).collect::<Vec<_>>());
        }
    }

    #[test]
    fn shuffle_large_queue_full_pass() {
        // 50k-track queue: one complete pass plays every track exactly once.
        const N: i64 = 50_000;
        let mut r = rng(7);
        let mut o = ShuffleOrder::new(N, 12_345, &mut r);
        assert!(o.is_consistent(N));
        let mut seen = vec![false; N as usize];
        seen[o.current().unwrap() as usize] = true;
        let mut count = 1;
        while let Some(i) = o.advance(&RepeatMode::Off, &mut r) {
            assert!(!seen[i as usize], "track {i} repeated");
            seen[i as usize] = true;
            count += 1;
        }
        assert_eq!(count, N);
    }

    #[test]
    fn shuffle_actually_shuffles() {
        // With 20 tracks, a seeded order equal to 0..20 would mean no shuffling.
        let o = ShuffleOrder::new(20, 0, &mut rng(1));
        assert_ne!(o.order, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_prev_retraces_next() {
        let mut r = rng(42);
        let mut o = ShuffleOrder::new(8, 0, &mut r);
        let mut forward = vec![o.current().unwrap()];
        for _ in 0..5 {
            forward.push(o.advance(&RepeatMode::Off, &mut r).unwrap());
        }
        let mut backward = vec![o.current().unwrap()];
        while let Some(i) = o.prev() {
            backward.push(i);
        }
        backward.reverse();
        assert_eq!(forward, backward);
        assert_eq!(o.prev(), None, "prev at start returns None");
    }

    #[test]
    fn shuffle_repeat_one_stays() {
        let mut r = rng(5);
        let mut o = ShuffleOrder::new(5, 2, &mut r);
        assert_eq!(o.advance(&RepeatMode::One, &mut r), Some(2));
        assert_eq!(o.pos, 0);
    }

    #[test]
    fn shuffle_repeat_all_reshuffles_without_repeating_last_first() {
        for seed in 0..100 {
            let mut r = rng(seed);
            let mut o = ShuffleOrder::new(4, 0, &mut r);
            for _ in 0..3 {
                o.advance(&RepeatMode::All, &mut r);
            }
            let last = o.current();
            let first_of_next_pass = o.advance(&RepeatMode::All, &mut r);
            assert_ne!(first_of_next_pass, last);
            assert_eq!(o.pos, 0);
            assert!(o.is_consistent(4));
        }
    }

    #[test]
    fn shuffle_single_track() {
        let mut r = rng(0);
        let mut o = ShuffleOrder::new(1, 0, &mut r);
        assert_eq!(o.advance(&RepeatMode::Off, &mut r), None);
        assert_eq!(o.advance(&RepeatMode::All, &mut r), Some(0));
    }

    #[test]
    fn shuffle_jump_to_keeps_no_repeats() {
        let mut r = rng(9);
        let mut o = ShuffleOrder::new(10, 0, &mut r);
        o.advance(&RepeatMode::Off, &mut r);
        // Jump to a track later in the order.
        let target = *o.order.last().unwrap();
        o.jump_to(target);
        assert_eq!(o.current(), Some(target));
        assert!(o.is_consistent(10));
        let mut played: Vec<i64> = o.order[..=o.pos].to_vec();
        while let Some(i) = o.advance(&RepeatMode::Off, &mut r) {
            played.push(i);
        }
        played.sort();
        assert_eq!(played, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn shuffle_jump_to_already_played_track() {
        let mut r = rng(11);
        let mut o = ShuffleOrder::new(6, 0, &mut r);
        let first = o.current().unwrap();
        o.advance(&RepeatMode::Off, &mut r);
        o.advance(&RepeatMode::Off, &mut r);
        o.jump_to(first);
        assert_eq!(o.current(), Some(first));
        assert!(o.is_consistent(6));
    }

    #[test]
    fn shuffle_insert_and_remove_stay_consistent() {
        let mut r = rng(3);
        let mut o = ShuffleOrder::new(5, 0, &mut r);
        o.advance(&RepeatMode::Off, &mut r);
        let current = o.current().unwrap();

        // Append a track (queue index 5): lands after the cursor, unplayed.
        o.on_insert(5, &mut r);
        assert!(o.is_consistent(6));
        assert_eq!(o.current(), Some(current));
        assert!(o.order[o.pos + 1..].contains(&5));

        // Remove a track that isn't current.
        let other = o.order.iter().copied().find(|&x| x != current).unwrap();
        o.on_remove(other);
        assert!(o.is_consistent(5));
        let current_after = if current > other { current - 1 } else { current };
        assert_eq!(o.current(), Some(current_after));
    }

    #[test]
    fn shuffle_remove_current_moves_to_next_in_order() {
        let mut r = rng(4);
        let mut o = ShuffleOrder::new(5, 0, &mut r);
        let current = o.current().unwrap();
        let next = o.order[1];
        o.on_remove(current);
        assert!(o.is_consistent(4));
        let expected = if next > current { next - 1 } else { next };
        assert_eq!(o.current(), Some(expected));
    }

    #[test]
    fn shuffle_remove_last_entry_clamps_cursor() {
        let mut r = rng(6);
        let mut o = ShuffleOrder::new(3, 0, &mut r);
        o.advance(&RepeatMode::Off, &mut r);
        o.advance(&RepeatMode::Off, &mut r);
        let current = o.current().unwrap();
        o.on_remove(current);
        assert!(o.is_consistent(2));
        assert_eq!(o.pos, 1);
    }

    #[test]
    fn shuffle_consistency_detects_stale_order() {
        let o = ShuffleOrder::new(5, 0, &mut rng(0));
        assert!(!o.is_consistent(6));
        let bad = ShuffleOrder { order: vec![0, 0, 1], pos: 0 };
        assert!(!bad.is_consistent(3));
        let empty = ShuffleOrder { order: vec![], pos: 0 };
        assert!(empty.is_consistent(0));
    }

    #[test]
    fn shuffle_order_json_roundtrip() {
        let o = ShuffleOrder::new(6, 2, &mut rng(8));
        let json = serde_json::to_string(&o).unwrap();
        assert_eq!(serde_json::from_str::<ShuffleOrder>(&json).unwrap(), o);
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
