use crate::audio::PlayerHandle;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

const FADE_SECS: u64 = 10;
const FADE_DUR: Duration = Duration::from_secs(FADE_SECS);

pub struct SleepTimerInner {
    pub active: bool,
    pub end_of_song: bool,
    pub deadline: Option<Instant>,
    pub fade: bool,
    pub fade_started_at: Option<Instant>,
    pub pre_fade_volume: f32,
}

impl SleepTimerInner {
    fn reset(&mut self) {
        self.active = false;
        self.end_of_song = false;
        self.deadline = None;
        self.fade = false;
        self.fade_started_at = None;
        self.pre_fade_volume = 0.75;
    }
}

pub struct SleepTimer(pub Mutex<SleepTimerInner>);

impl SleepTimer {
    pub fn new() -> Self {
        Self(Mutex::new(SleepTimerInner {
            active: false,
            end_of_song: false,
            deadline: None,
            fade: false,
            fade_started_at: None,
            pre_fade_volume: 0.75,
        }))
    }
}

#[derive(Serialize, Clone)]
pub struct SleepTimerInfo {
    pub active: bool,
    pub end_of_song: bool,
    pub remaining_secs: Option<u64>,
    pub fade: bool,
}

pub fn get_info(st: &SleepTimerInner) -> SleepTimerInfo {
    if !st.active {
        return SleepTimerInfo {
            active: false,
            end_of_song: false,
            remaining_secs: None,
            fade: false,
        };
    }
    let remaining_secs = st.deadline.map(|d| {
        let now = Instant::now();
        if d > now { (d - now).as_secs() } else { 0 }
    });
    SleepTimerInfo {
        active: true,
        end_of_song: st.end_of_song,
        remaining_secs,
        fade: st.fade,
    }
}

fn emit_info(app: &AppHandle) {
    if let Ok(st) = app.state::<SleepTimer>().0.lock() {
        let _ = app.emit("sleep-timer", get_info(&st));
    }
}

#[tauri::command]
pub fn set_sleep_timer(app: AppHandle, minutes: u64, fade: bool) -> Result<(), String> {
    {
        let timer = app.state::<SleepTimer>();
        let mut st = timer.0.lock().map_err(|e| e.to_string())?;
        st.reset();
        st.active = true;
        st.end_of_song = false;
        st.fade = fade;
        st.deadline = Some(Instant::now() + Duration::from_secs(minutes * 60));
    }
    emit_info(&app);
    Ok(())
}

#[tauri::command]
pub fn set_sleep_timer_end_of_song(app: AppHandle, fade: bool) -> Result<(), String> {
    {
        let timer = app.state::<SleepTimer>();
        let mut st = timer.0.lock().map_err(|e| e.to_string())?;
        st.reset();
        st.active = true;
        st.end_of_song = true;
        st.fade = fade;
    }
    emit_info(&app);
    Ok(())
}

#[tauri::command]
pub fn cancel_sleep_timer(app: AppHandle) -> Result<(), String> {
    {
        let timer = app.state::<SleepTimer>();
        let player = app.state::<PlayerHandle>();
        let mut st = timer.0.lock().map_err(|e| e.to_string())?;
        if st.fade_started_at.is_some() {
            player.set_volume(st.pre_fade_volume);
        }
        st.reset();
    }
    emit_info(&app);
    Ok(())
}

#[tauri::command]
pub fn get_sleep_timer(timer: State<SleepTimer>) -> Result<SleepTimerInfo, String> {
    let st = timer.0.lock().map_err(|e| e.to_string())?;
    Ok(get_info(&st))
}

/// Called every 250 ms by the ticker. Drives the deadline countdown and fade-out volume ramp.
pub fn tick(app: &AppHandle) {
    let timer = app.state::<SleepTimer>();
    let player = app.state::<PlayerHandle>();

    let mut st = match timer.0.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if !st.active {
        return;
    }

    if st.end_of_song {
        // Start pre-fade when the track enters its last FADE_SECS.
        if st.fade && st.fade_started_at.is_none() {
            let snap = player.snapshot();
            if let Some(dur) = snap.duration_ms {
                let remaining = dur.saturating_sub(snap.position_ms);
                if snap.track_id.is_some() && remaining > 0 && remaining <= FADE_SECS * 1000 {
                    st.fade_started_at = Some(Instant::now());
                    st.pre_fade_volume = snap.volume;
                }
            }
        }
        if let Some(started) = st.fade_started_at {
            let progress =
                (started.elapsed().as_millis() as f32 / FADE_DUR.as_millis() as f32).min(1.0);
            player.set_volume((st.pre_fade_volume * (1.0 - progress)).max(0.0));
        }
        return; // actual stop handled in handle_track_end()
    }

    // Fixed-deadline mode
    let Some(deadline) = st.deadline else {
        return;
    };
    let now = Instant::now();

    // Enter fade phase when within FADE_SECS of the deadline.
    if st.fade && st.fade_started_at.is_none() && now + FADE_DUR >= deadline {
        let snap = player.snapshot();
        st.fade_started_at = Some(now);
        st.pre_fade_volume = snap.volume;
    }

    if let Some(started) = st.fade_started_at {
        let progress =
            (started.elapsed().as_millis() as f32 / FADE_DUR.as_millis() as f32).min(1.0);
        player.set_volume((st.pre_fade_volume * (1.0 - progress)).max(0.0));
    }

    if now >= deadline {
        let pre_fade = st.pre_fade_volume;
        let was_fading = st.fade_started_at.is_some();
        st.reset();
        drop(st);
        player.stop();
        if was_fading {
            player.set_volume(pre_fade);
        }
        emit_info(app);
        // player-state is re-emitted by the ticker loop immediately after this returns
    }
}

/// Called from `on_track_finished` when a track ends naturally.
/// Returns true if the sleep timer consumed the end — caller must NOT load the next track.
pub fn handle_track_end(app: &AppHandle) -> bool {
    let timer = app.state::<SleepTimer>();
    let player = app.state::<PlayerHandle>();

    let mut st = match timer.0.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };

    if !st.active || !st.end_of_song {
        return false;
    }

    let pre_fade = if st.fade_started_at.is_some() {
        st.pre_fade_volume
    } else {
        player.snapshot().volume
    };
    st.reset();
    drop(st);
    player.stop();
    player.set_volume(pre_fade);
    emit_info(app);
    true
}
