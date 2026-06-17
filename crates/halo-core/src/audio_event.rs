#[derive(Debug, Clone)]
pub enum AudioEvent {
    /// Audio data for the current track was fully consumed by the decoder.
    /// `track_id` is captured before the Worker clears its internal state.
    TrackFinished { track_id: i64 },
    /// Remaining audio dropped below the near-end threshold (≤ 15 s).
    /// Sent once per track load; the app layer decides whether to crossfade.
    NearEnd { track_id: i64, remaining_ms: u64 },
}
