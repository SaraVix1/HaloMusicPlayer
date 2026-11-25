//! Application state, message routing and window management.

use std::time::Duration;

use halo_backend::audio::PlaybackStatus;
use halo_backend::commands::{self, player::FullPlayerState};
use halo_backend::{Backend, BackendEvent};
use halo_core::queue::RepeatMode;
use iced::keyboard::{self, key, Key, Modifiers};
use iced::widget::{column, container, mouse_area, row, stack};
use iced::{event, mouse, window, Element, Length, Point, Size, Subscription, Task, Theme};

use crate::components::{context_menu, drop, fullscreen, lyrics, mini, now_playing, palette, sidebar, title_bar, queue_panel};
use crate::pages;
use crate::route::{History, Route};
use crate::theme;
use crate::tray::{self, TrayAction};

/// Run `f` on the blocking thread pool and deliver its result to `then`.
pub fn bg<T, M>(
    backend: &Backend,
    f: impl FnOnce(Backend) -> T + Send + 'static,
    then: impl FnOnce(T) -> M + Send + 'static,
) -> Task<M>
where
    T: Send + 'static,
    M: Send + 'static,
{
    let b = backend.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || f(b))
                .await
                .expect("background task panicked")
        },
        then,
    )
}

/// Fire-and-forget backend call; errors are logged.
pub fn fire(
    backend: &Backend,
    f: impl FnOnce(Backend) -> Result<(), String> + Send + 'static,
) -> Task<Message> {
    bg(backend, f, |r| match r {
        Ok(()) => Message::Noop,
        Err(e) => Message::Error(e),
    })
}

#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    /// Accent picked from a track's art (track id, colour or none).
    AccentPicked(Option<i64>, Option<iced::Color>),
    Error(String),
    /// Several messages handled in order.
    Batch(Vec<Message>),
    Backend(BackendEvent),

    // Navigation
    Navigate(Route),
    ReplaceRoute(Route),
    Back,
    Forward,

    // Windows
    WindowOpened(window::Id),
    WindowEvent(window::Id, window::Event),
    CloseRequested(window::Id),
    /// Close button with "minimize to tray": is there a tray to hide into?
    CloseToTray(bool),
    DragWindow(window::Id),
    ResizeWindow(window::Id, window::Direction),
    Minimize,
    ToggleMaximize,
    CloseMain,
    OpenMini,

    // Input / system
    Key(Key, Modifiers, window::Id),
    SystemTheme(iced::theme::Mode),
    Tray(TrayAction),

    // Player transport (shared by now-playing bar, mini player, tray, pages)
    Player(PlayerCmd),

    NowPlaying(now_playing::Msg),
    Fullscreen(fullscreen::Msg),
    QueuePanel(queue_panel::Msg),
    Context(context_menu::Msg),
    Drop(drop::Msg),
    Mini(mini::Msg),
    Lyrics(lyrics::Msg),
    Palette(palette::Msg),
    Page(pages::Msg),
}

#[derive(Debug, Clone)]
pub enum PlayerCmd {
    TogglePlay,
    Next,
    Previous,
    Seek(u64),
    SetVolume(f32),
    ToggleMute,
    SetShuffle(bool),
    CycleRepeat,
    SetRating(i64, i64),
    PlayQueueIndex(i64),
    PlayTracks(Vec<i64>, i64),
    ShufflePlay(Vec<i64>),
    AddToQueue(i64),
}

pub struct Halo {
    pub backend: Backend,
    pub main_id: Option<window::Id>,
    pub main_size: Size,
    pub main_visible: bool,
    pub mini_id: Option<window::Id>,

    pub theme_pref: String,
    /// Settings → Appearance → "Accent from album art".
    pub accent_from_art: bool,
    pub system_mode: iced::theme::Mode,

    pub player: FullPlayerState,
    pub history: History,

    pub now_playing: now_playing::State,
    pub mini: mini::State,
    pub lyrics: lyrics::State,
    pub fullscreen: fullscreen::State,
    pub queue_panel: queue_panel::State,
    pub context: context_menu::State,
    pub drop: drop::State,
    pub palette: palette::State,
    pub pages: pages::Pages,
    media_started: bool,
}

fn empty_state() -> FullPlayerState {
    FullPlayerState {
        player: halo_backend::audio::PlayerState::default(),
        current_index: None,
        queue_length: 0,
        shuffle: false,
        repeat: RepeatMode::Off,
        crossfade_ms: 0,
        current_track: None,
        sleep_timer: commands::sleep_timer::SleepTimerInfo {
            active: false,
            end_of_song: false,
            remaining_secs: None,
            fade: false,
        },
    }
}

pub fn main_window_settings() -> window::Settings {
    window::Settings {
        size: Size::new(1200.0, 750.0),
        min_size: Some(Size::new(900.0, 600.0)),
        decorations: false,
        visible: true,
        exit_on_close_request: false,
        icon: crate::app_icon(),
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: halo_backend::APP_IDENTIFIER.into(),
            ..Default::default()
        },
        #[cfg(windows)]
        platform_specific: window::settings::PlatformSpecific {
            undecorated_shadow: true,
            ..Default::default()
        },
        ..window::Settings::default()
    }
}

/// `app_state` key for Settings → Appearance → "Accent from album art".
pub const ACCENT_PREF: &str = "ui.accent_from_art";

impl Halo {
    pub fn new(backend: Backend) -> (Self, Task<Message>) {
        let theme_pref = backend.initial_theme().to_string();
        let player = commands::player::get_player_state(backend.clone()).unwrap_or_else(|_| empty_state());
        let (main_id, open) = window::open(main_window_settings());
        let mut app = Self {
            backend: backend.clone(),
            main_id: Some(main_id),
            main_size: Size::new(1200.0, 750.0),
            main_visible: true,
            mini_id: None,
            theme_pref,
            accent_from_art: commands::ui::get_pref(ACCENT_PREF.into(), backend.db())
                .ok()
                .flatten()
                .as_deref()
                != Some("false"),
            system_mode: iced::theme::Mode::Dark,
            player,
            history: History::default(),
            now_playing: now_playing::State::default(),
            mini: mini::State::default(),
            lyrics: lyrics::State::default(),
            fullscreen: fullscreen::State::default(),
            queue_panel: queue_panel::State::default(),
            context: context_menu::State::default(),
            drop: drop::State::default(),
            palette: palette::State::default(),
            pages: pages::Pages {
                library: pages::library::LibraryState::new(&backend),
                ..Default::default()
            },
            media_started: false,
        };
        app.apply_theme();
        let track_task = app.on_track_changed();
        let page_task = pages::on_enter(&mut app);
        (
            app,
            Task::batch([
                open.map(Message::WindowOpened),
                iced::system::theme().map(Message::SystemTheme),
                track_task,
                page_task,
            ]),
        )
    }

    pub fn route(&self) -> &Route {
        self.history.current()
    }

    pub fn is_playing(&self) -> bool {
        self.player.player.status == PlaybackStatus::Playing
    }

    /// Change the theme preference (light / dark / system) and apply it now.
    pub fn set_theme_pref(&mut self, pref: String) {
        self.theme_pref = pref;
        self.apply_theme();
    }

    /// Light / dark from the preference; always dark while full-screen now
    /// playing is open (light text over the darkened art).
    pub fn apply_theme(&self) {
        let dark = self.fullscreen.open
            || match self.theme_pref.as_str() {
                "light" => false,
                "system" => self.system_mode != iced::theme::Mode::Light,
                _ => true,
            };
        theme::set_dark(dark);
    }

    pub fn theme(&self, _id: window::Id) -> Theme {
        theme::iced_theme()
    }

    pub fn title(&self, id: window::Id) -> String {
        if Some(id) == self.mini_id {
            "Halo Mini".into()
        } else {
            "Halo".into()
        }
    }

    /// Track changed → refetch waveform, Last.fm loved state, lyrics.
    fn on_track_changed(&mut self) -> Task<Message> {
        let id = self.player.current_track.as_ref().map(|t| t.track_id);
        Task::batch([
            now_playing::on_track(self, id),
            lyrics::on_track(self, id),
            fullscreen::on_track(self),
            self.refresh_accent(),
        ])
    }

    /// Pick the accent from the current track's art (off the UI thread), or
    /// use Halo's default when there's no art or the setting is off.
    pub fn refresh_accent(&self) -> Task<Message> {
        let cur = self.player.current_track.as_ref();
        let art = cur.and_then(|t| t.album_art_path.clone()).filter(|p| !p.is_empty());
        match art {
            Some(path) if self.accent_from_art => {
                let id = cur.map(|t| t.track_id);
                bg(&self.backend, move |_| crate::accent::from_path(&path), move |c| Message::AccentPicked(id, c))
            }
            _ => {
                theme::set_accent(theme::DEFAULT_ACCENT);
                Task::none()
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
            Message::AccentPicked(id, c) => {
                // Ignore a late result for a track that's no longer playing.
                if self.accent_from_art && id == self.player.current_track.as_ref().map(|t| t.track_id) {
                    theme::set_accent(c.unwrap_or(theme::DEFAULT_ACCENT));
                }
                Task::none()
            }
            Message::Error(e) => {
                eprintln!("error: {e}");
                Task::none()
            }
            Message::Batch(msgs) => {
                Task::batch(msgs.into_iter().map(|m| self.update(m)).collect::<Vec<_>>())
            }
            Message::Backend(ev) => self.on_backend_event(ev),

            Message::Navigate(route) => {
                let prev = self.route().clone();
                self.lyrics.open = false;
                // e.g. a Ctrl+K result picked over full-screen now playing.
                let leave = fullscreen::update(self, fullscreen::Msg::Close);
                context_menu::close(self);
                self.history.push(route);
                Task::batch([leave, pages::on_navigate(self, prev)])
            }
            Message::ReplaceRoute(route) => {
                let prev = self.route().clone();
                self.history.replace(route);
                pages::on_navigate(self, prev)
            }
            Message::Back => {
                let prev = self.route().clone();
                if self.history.back() {
                    self.lyrics.open = false;
                    pages::on_navigate(self, prev)
                } else {
                    Task::none()
                }
            }
            Message::Forward => {
                let prev = self.route().clone();
                if self.history.forward() {
                    self.lyrics.open = false;
                    pages::on_navigate(self, prev)
                } else {
                    Task::none()
                }
            }

            Message::WindowOpened(id) => {
                if Some(id) == self.main_id && !self.media_started {
                    self.media_started = true;
                    tray::create();
                    return self.start_media_controls(id);
                }
                Task::none()
            }
            Message::WindowEvent(id, ev) => self.on_window_event(id, ev),
            Message::CloseRequested(id) => {
                if Some(id) == self.mini_id {
                    return self.restore_main();
                }
                let behavior = self
                    .backend
                    .db()
                    .lock()
                    .map(|c| commands::ui::read_close_behavior(&c))
                    .unwrap_or_else(|_| "minimize".into());
                if behavior == "quit" {
                    iced::exit()
                } else {
                    // Only hide if a tray can bring the window back (plain
                    // GNOME has none); otherwise minimize to the dock.
                    Task::perform(
                        async { tokio::task::spawn_blocking(tray::host_available).await.unwrap_or(true) },
                        Message::CloseToTray,
                    )
                }
            }
            Message::CloseToTray(has_tray) => {
                if has_tray {
                    self.hide_main()
                } else {
                    eprintln!("no system tray available: minimizing instead of hiding to tray");
                    self.main_id.map(|id| window::minimize(id, true)).unwrap_or_else(Task::none)
                }
            }
            Message::DragWindow(id) => window::drag(id),
            Message::ResizeWindow(id, dir) => window::drag_resize(id, dir),
            Message::Minimize => self.main_id.map(|id| window::minimize(id, true)).unwrap_or_else(Task::none),
            Message::ToggleMaximize => self.main_id.map(window::toggle_maximize).unwrap_or_else(Task::none),
            Message::CloseMain => match self.main_id {
                Some(id) => self.update(Message::CloseRequested(id)),
                None => Task::none(),
            },
            Message::OpenMini => self.open_mini(),

            Message::Key(key, mods, id) => self.on_key(key, mods, id),
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                self.apply_theme();
                Task::none()
            }
            Message::Tray(action) => match action {
                TrayAction::PlayPause => self.update(Message::Player(PlayerCmd::TogglePlay)),
                TrayAction::Next => self.update(Message::Player(PlayerCmd::Next)),
                TrayAction::Previous => self.update(Message::Player(PlayerCmd::Previous)),
                TrayAction::Show => self.show_main(),
                TrayAction::Mini => self.open_mini(),
                TrayAction::Quit => iced::exit(),
            },

            Message::Player(cmd) => self.player_cmd(cmd),
            Message::NowPlaying(m) => now_playing::update(self, m),
            Message::Fullscreen(m) => fullscreen::update(self, m),
            Message::QueuePanel(m) => queue_panel::update(self, m),
            Message::Context(m) => context_menu::update(self, m),
            Message::Drop(m) => drop::update(self, m),
            Message::Mini(m) => mini::update(self, m),
            Message::Lyrics(m) => lyrics::update(self, m),
            Message::Palette(m) => palette::update(self, m),
            Message::Page(m) => pages::update(self, m),
        }
    }

    fn on_backend_event(&mut self, ev: BackendEvent) -> Task<Message> {
        match ev {
            BackendEvent::PlayerState(s) => {
                let prev_track = self.player.current_track.as_ref().map(|t| t.track_id);
                let prev_len = self.player.queue_length;
                let prev_shuffle = self.player.shuffle;
                self.player = *s;
                let mut tasks = vec![];
                // Up next depends on the track, the queue and the shuffle order.
                if self.player.current_track.as_ref().map(|t| t.track_id) != prev_track
                    || self.player.queue_length != prev_len
                    || self.player.shuffle != prev_shuffle
                {
                    tasks.push(queue_panel::on_player_changed(self));
                }
                if self.player.current_track.as_ref().map(|t| t.track_id) != prev_track {
                    tasks.push(self.on_track_changed());
                }
                tasks.push(lyrics::on_position(self));
                Task::batch(tasks)
            }
            BackendEvent::ThemeChanged(theme) => {
                self.theme_pref = theme;
                self.apply_theme();
                Task::none()
            }
            BackendEvent::RaiseMain => self.show_main(),
            BackendEvent::RevealMain => {
                if self.mini_id.is_some() {
                    Task::none()
                } else {
                    self.show_main()
                }
            }
            BackendEvent::LibraryChanged => {
                crate::ui::invalidate_art_cache();
                pages::on_library_changed(self)
            }
            BackendEvent::ScanProgress(p) => pages::on_scan_progress(self, p),
            BackendEvent::EqStateChanged(cfg) => pages::on_eq_changed(self, cfg),
            BackendEvent::SleepTimer(info) => {
                self.player.sleep_timer = info;
                Task::none()
            }
            BackendEvent::RatingChanged { track_id, rating } => {
                pages::on_rating_changed(self, track_id, rating)
            }
        }
    }

    fn player_cmd(&mut self, cmd: PlayerCmd) -> Task<Message> {
        use commands::player as p;
        let b = &self.backend;
        match cmd {
            PlayerCmd::TogglePlay => fire(b, p::toggle_play_pause),
            PlayerCmd::Next => fire(b, p::next_track),
            PlayerCmd::Previous => fire(b, p::previous_track),
            PlayerCmd::Seek(ms) => {
                self.player.player.position_ms = ms;
                fire(b, move |b| p::seek_to(b, ms))
            }
            PlayerCmd::SetVolume(v) => {
                self.player.player.volume = v;
                fire(b, move |b| p::set_volume(b, v))
            }
            PlayerCmd::ToggleMute => {
                let v = if self.player.player.volume > 0.0 { 0.0 } else { 0.75 };
                self.player.player.volume = v;
                fire(b, move |b| p::set_volume(b, v))
            }
            PlayerCmd::SetShuffle(on) => {
                self.player.shuffle = on;
                fire(b, move |b| p::set_shuffle(b, on))
            }
            PlayerCmd::CycleRepeat => {
                let next = match self.player.repeat {
                    RepeatMode::Off => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::Off,
                };
                self.player.repeat = next.clone();
                fire(b, move |b| p::set_repeat(b, next))
            }
            PlayerCmd::SetRating(track_id, rating) => {
                if let Some(t) = self.player.current_track.as_mut() {
                    if t.track_id == track_id {
                        t.rating = rating;
                    }
                }
                let task = pages::on_rating_changed(self, track_id, rating);
                Task::batch([
                    task,
                    fire(&self.backend, move |b| commands::stats::set_rating(b, track_id, rating)),
                ])
            }
            PlayerCmd::PlayQueueIndex(i) => fire(b, move |b| p::play_queue_index(b, i)),
            PlayerCmd::PlayTracks(ids, start) => {
                fire(b, move |b| p::set_queue_and_play(b, ids, start))
            }
            PlayerCmd::ShufflePlay(ids) => fire(b, move |b| {
                if ids.is_empty() {
                    return Ok(());
                }
                p::set_shuffle(b.clone(), true)?;
                let start = random_index(ids.len());
                p::set_queue_and_play(b, ids, start as i64)
            }),
            PlayerCmd::AddToQueue(id) => fire(b, move |b| p::add_to_queue(b, id)),
        }
    }

    fn on_key(&mut self, key: Key, mods: Modifiers, id: window::Id) -> Task<Message> {
        let cmd = mods.command() || mods.control();
        if Some(id) == self.mini_id {
            if cmd && matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("m")) {
                return self.restore_main();
            }
            return Task::none();
        }
        match &key {
            Key::Character(c) if cmd && c.eq_ignore_ascii_case("k") => {
                palette::toggle(self)
            }
            Key::Character(c) if cmd && c.eq_ignore_ascii_case("m") => self.open_mini(),
            Key::Named(key::Named::Escape) if drop::is_open(self) => self.update(Message::Drop(drop::Msg::Cancel)),
            Key::Named(key::Named::Enter) if drop::is_open(self) && !self.palette.open => drop::on_enter(self),
            Key::Named(key::Named::Escape) if context_menu::is_open(self) => {
                self.update(Message::Context(context_menu::Msg::Close))
            }
            Key::Named(key::Named::Escape) if self.palette.open => {
                self.update(Message::Palette(palette::Msg::Close))
            }
            Key::Named(key::Named::Escape) if self.fullscreen.open && !pages::has_dialog(self) => {
                self.update(Message::Fullscreen(fullscreen::Msg::Close))
            }
            Key::Named(key::Named::F11) if self.fullscreen.open => {
                self.update(Message::Fullscreen(fullscreen::Msg::ToggleOsFullscreen))
            }
            Key::Named(key::Named::Escape) if self.queue_panel.open && !pages::has_dialog(self) => {
                self.update(Message::QueuePanel(queue_panel::Msg::Close))
            }
            Key::Named(key::Named::Escape) => pages::on_escape(self),
            Key::Named(key::Named::ArrowUp) if self.palette.open => {
                self.update(Message::Palette(palette::Msg::Move(-1)))
            }
            Key::Named(key::Named::ArrowDown) if self.palette.open => {
                self.update(Message::Palette(palette::Msg::Move(1)))
            }
            // Enter in the palette: its text field submits (Shift / Ctrl read
            // from the palette's own modifier tracking).
            Key::Named(key::Named::Enter) if self.palette.open => Task::none(),
            Key::Named(key::Named::Enter) if cmd => pages::on_ctrl_enter(self),
            _ => Task::none(),
        }
    }

    fn on_window_event(&mut self, id: window::Id, ev: window::Event) -> Task<Message> {
        match ev {
            window::Event::Resized(size) if Some(id) == self.main_id => {
                self.main_size = size;
                Task::none()
            }
            window::Event::Moved(pos) if Some(id) == self.mini_id => mini::on_moved(self, pos),
            window::Event::Closed if Some(id) == self.mini_id => {
                self.mini_id = None;
                Task::none()
            }
            window::Event::Closed if Some(id) == self.main_id => iced::exit(),
            window::Event::FileHovered(_) if Some(id) == self.main_id => drop::update(self, drop::Msg::Hovered),
            window::Event::FilesHoveredLeft if Some(id) == self.main_id => drop::update(self, drop::Msg::Left),
            window::Event::FileDropped(p) if Some(id) == self.main_id => drop::update(self, drop::Msg::Dropped(p)),
            _ => Task::none(),
        }
    }

    fn start_media_controls(&mut self, id: window::Id) -> Task<Message> {
        #[cfg(windows)]
        {
            let backend = self.backend.clone();
            window::run(id, move |w| {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let hwnd = w.window_handle().ok().and_then(|h| match h.as_raw() {
                    RawWindowHandle::Win32(h) => Some(h.hwnd.get() as *mut std::ffi::c_void),
                    _ => None,
                });
                // SAFETY of the pointer is upheld by souvlaki: it only uses
                // the HWND while the (never-destroyed) main window exists.
                backend.start_media_controls(hwnd);
            })
            .discard()
        }
        #[cfg(not(windows))]
        {
            let _ = id;
            self.backend.start_media_controls(None);
            Task::none()
        }
    }

    pub fn hide_main(&mut self) -> Task<Message> {
        self.main_visible = false;
        self.main_id
            .map(|id| window::set_mode(id, window::Mode::Hidden))
            .unwrap_or_else(Task::none)
    }

    pub fn show_main(&mut self) -> Task<Message> {
        let close_mini = self.mini_id.take().map(window::close).unwrap_or_else(Task::none);
        self.main_visible = true;
        let show = self
            .main_id
            .map(|id| {
                Task::batch([
                    window::set_mode(id, window::Mode::Windowed),
                    window::minimize(id, false),
                    window::gain_focus(id),
                ])
            })
            .unwrap_or_else(Task::none);
        Task::batch([close_mini, show])
    }

    pub fn restore_main(&mut self) -> Task<Message> {
        self.show_main()
    }

    pub fn open_mini(&mut self) -> Task<Message> {
        let leave = fullscreen::update(self, fullscreen::Msg::Close);
        let open = mini::open(self);
        Task::batch([leave, open, self.hide_main()])
    }

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        if Some(id) == self.mini_id {
            return mini::view(self);
        }
        let main_id = id;
        if self.fullscreen.open {
            let mut layers = stack![fullscreen::view(self, title_bar::view(self, main_id))]
                .width(Length::Fill)
                .height(Length::Fill);
            if let Some(overlay) = pages::overlay(self) {
                layers = layers.push(overlay);
            }
            if self.palette.open {
                layers = layers.push(palette::view(self));
            }
            if let Some(d) = drop::dialog(self) {
                layers = layers.push(d);
            }
            if self.drop.hovering {
                layers = layers.push(drop::overlay());
            }
            if let Some(menu) = context_menu::view(self) {
                layers = layers.push(menu);
            }
            return layers.push(context_menu::probe()).push(resize_handles(main_id)).into();
        }
        let content: Element<'_, Message> = if self.lyrics.open {
            lyrics::view(self)
        } else {
            pages::view(self)
        };
        let page = container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::page);
        // The queue panel slides in over the page, right edge.
        let page: Element<'_, Message> = if queue_panel::visible(self) {
            stack![page, queue_panel::view(self)].width(Length::Fill).height(Length::Fill).into()
        } else {
            page.into()
        };
        let body = row![sidebar::view(self), page].height(Length::Fill);

        let shell = column![title_bar::view(self, main_id), body, now_playing::view(self)]
            .width(Length::Fill)
            .height(Length::Fill);

        let mut layers = stack![container(shell).style(theme::page)]
            .width(Length::Fill)
            .height(Length::Fill);
        if let Some(overlay) = pages::overlay(self) {
            layers = layers.push(overlay);
        }
        if self.palette.open {
            layers = layers.push(palette::view(self));
        }
        if let Some(d) = drop::dialog(self) {
            layers = layers.push(d);
        }
        if self.drop.hovering {
            layers = layers.push(drop::overlay());
        }
        if let Some(menu) = context_menu::view(self) {
            layers = layers.push(menu);
        }
        layers = layers.push(context_menu::probe());
        layers = layers.push(resize_handles(main_id));
        layers.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            Subscription::run_with(BackendStream(self.backend.clone()), backend_events)
                .map(Message::Backend),
            window::close_requests().map(Message::CloseRequested),
            window::events().map(|(id, e)| Message::WindowEvent(id, e)),
            iced::system::theme_changes().map(Message::SystemTheme),
            tray::events().map(Message::Tray),
            event::listen_with(|e, status, id| match e {
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    let has_cmd = modifiers.command() || modifiers.control();
                    // Shortcuts with Ctrl/Cmd fire even from text fields;
                    // plain keys only when nothing captured them.
                    // Escape always (closes dialogs / palette from inputs).
                    let escape = matches!(key, Key::Named(key::Named::Escape));
                    (has_cmd || escape || status == event::Status::Ignored)
                        .then_some(Message::Key(key, modifiers, id))
                }
                _ => None,
            }),
        ];
        if let Some(s) = pages::subscription(self) {
            subs.push(s);
        }
        if let Some(s) = lyrics::subscription(self) {
            subs.push(s);
        }
        if let Some(s) = mini::subscription(self) {
            subs.push(s);
        }
        if let Some(s) = queue_panel::subscription(self) {
            subs.push(s);
        }
        if let Some(s) = palette::subscription(self) {
            subs.push(s);
        }
        Subscription::batch(subs)
    }
}

/// Random index in `0..len` (Math.random equivalent for shuffle-play starts).
pub fn random_index(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len.max(1)
}

/// Invisible 6px edges/corners on the undecorated main window that start an
/// OS resize drag (Tauri's undecorated windows were resizable too).
fn resize_handles<'a>(id: window::Id) -> Element<'a, Message> {
    use window::Direction as D;
    const E: f32 = 6.0;
    let edge = |dir: D, w: Length, h: Length, cursor: mouse::Interaction| -> Element<'a, Message> {
        mouse_area(container(iced::widget::space()).width(w).height(h))
            .on_press(Message::ResizeWindow(id, dir))
            .interaction(cursor)
            .into()
    };
    use iced::widget::{space, Column, Row};
    let top = Row::new()
        .push(edge(D::NorthWest, Length::Fixed(E), Length::Fixed(E), mouse::Interaction::ResizingDiagonallyDown))
        .push(edge(D::North, Length::Fill, Length::Fixed(E), mouse::Interaction::ResizingVertically))
        .push(edge(D::NorthEast, Length::Fixed(E), Length::Fixed(E), mouse::Interaction::ResizingDiagonallyUp));
    let bottom = Row::new()
        .push(edge(D::SouthWest, Length::Fixed(E), Length::Fixed(E), mouse::Interaction::ResizingDiagonallyUp))
        .push(edge(D::South, Length::Fill, Length::Fixed(E), mouse::Interaction::ResizingVertically))
        .push(edge(D::SouthEast, Length::Fixed(E), Length::Fixed(E), mouse::Interaction::ResizingDiagonallyDown));
    let middle = Row::new()
        .push(edge(D::West, Length::Fixed(E), Length::Fill, mouse::Interaction::ResizingHorizontally))
        .push(space().width(Length::Fill))
        .push(edge(D::East, Length::Fixed(E), Length::Fill, mouse::Interaction::ResizingHorizontally))
        .height(Length::Fill);
    Column::new()
        .push(top)
        .push(middle)
        .push(bottom)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ── Backend event stream ──────────────────────────────────────────────────────

struct BackendStream(Backend);

impl std::hash::Hash for BackendStream {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        "halo-backend-events".hash(state);
    }
}

fn backend_events(b: &BackendStream) -> impl iced::futures::Stream<Item = BackendEvent> {
    let rx = b.0.take_events();
    iced::stream::channel(64, async move |mut out| {
        use iced::futures::SinkExt;
        let Some(mut rx) = rx else {
            std::future::pending::<()>().await;
            return;
        };
        while let Some(ev) = rx.recv().await {
            if out.send(ev).await.is_err() {
                break;
            }
        }
    })
}

#[allow(dead_code)]
fn _unused(_: Point, _: Duration) {}
