//! Queue panel ("Up next"), toggled by ≡ in the now-playing bar: slides in
//! from the right over the page. Now playing, then what plays next in real
//! play order (the shuffle order when shuffle is on): drag ⠿ to reorder,
//! ✕ to remove, double-click to play, Clear keeps only the playing track.

use std::time::{Duration, Instant};

use halo_backend::commands::player::{self, QueueTrack, UpNext};
use iced::widget::{button, center, column, container, mouse_area, row, text, tooltip::Position, Space};
use iced::{event, mouse, Alignment, Element, Length, Padding, Subscription, Task};

use crate::app::{bg, Halo, Message, PlayerCmd};
use crate::format::plural;
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, art, icon_box, tip};
use crate::widgets::{line, surface};

pub const WIDTH: f32 = 360.0;
const ROW_H: f32 = 56.0;
const SLIDE: Duration = Duration::from_millis(180);
const SCROLL_KEY: &str = "queue-panel";

#[derive(Default)]
pub struct State {
    pub open: bool,
    /// Slide animation: when it started and towards which end.
    anim: Option<Instant>,
    data: UpNext,
    loaded: bool,
    error: Option<String>,
    drag: Option<usize>,
    over: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Toggle,
    Close,
    Tick,
    Loaded(Result<UpNext, String>),
    DragStart(usize),
    DragOver(usize),
    DragEnd,
    Remove(i64),
    Clear,
}

fn qm(m: Msg) -> Message {
    Message::QueuePanel(m)
}

pub fn toggle_message() -> Message {
    qm(Msg::Toggle)
}

/// 0 (hidden) … 1 (fully in), eased.
fn progress(st: &State) -> f32 {
    let p = match st.anim {
        Some(start) => (start.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).min(1.0),
        None => 1.0,
    };
    let eased = 1.0 - (1.0 - p).powi(3); // ease-out cubic
    if st.open { eased } else { 1.0 - eased }
}

fn animating(st: &State) -> bool {
    st.anim.is_some_and(|s| s.elapsed() < SLIDE)
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let st = &mut app.queue_panel;
    match m {
        Msg::Toggle => {
            st.open = !st.open;
            st.anim = Some(Instant::now());
            st.drag = None;
            if st.open { reload(app) } else { Task::none() }
        }
        Msg::Close => {
            if st.open {
                st.open = false;
                st.anim = Some(Instant::now());
                st.drag = None;
            }
            Task::none()
        }
        Msg::Tick => {
            if !animating(st) {
                st.anim = None;
            }
            Task::none()
        }
        Msg::Loaded(r) => {
            st.loaded = true;
            match r {
                Ok(d) => {
                    st.data = d;
                    st.error = None;
                }
                Err(e) => st.error = Some(e),
            }
            Task::none()
        }
        Msg::DragStart(i) => {
            st.drag = Some(i);
            st.over = Some(i);
            Task::none()
        }
        Msg::DragOver(i) => {
            if st.drag.is_some() {
                st.over = Some(i);
            }
            Task::none()
        }
        Msg::DragEnd => {
            let (Some(from), Some(to)) = (st.drag.take(), st.over.take()) else {
                return Task::none();
            };
            if from == to || from >= st.data.upcoming.len() || to >= st.data.upcoming.len() {
                return Task::none();
            }
            // Show the new order at once; the reload confirms it.
            let item = st.data.upcoming.remove(from);
            st.data.upcoming.insert(to, item);
            edit(app, move |b| player::move_up_next(b, from, to))
        }
        Msg::Remove(queue_id) => {
            st.data.upcoming.retain(|q| q.queue_id != queue_id);
            edit(app, move |b| player::remove_from_queue(b, queue_id))
        }
        Msg::Clear => {
            st.data.upcoming.clear();
            edit(app, player::clear_up_next)
        }
    }
}

/// Run a queue edit off the UI thread, then reload.
fn edit(app: &Halo, f: impl FnOnce(halo_backend::Backend) -> Result<(), String> + Send + 'static) -> Task<Message> {
    bg(
        &app.backend,
        move |b| {
            f(b.clone())?;
            player::get_up_next(&b)
        },
        |r| qm(Msg::Loaded(r)),
    )
}

fn reload(app: &Halo) -> Task<Message> {
    bg(&app.backend, |b| player::get_up_next(&b), |r| qm(Msg::Loaded(r)))
}

/// Player state changed in a way that changes Up next (track, queue
/// length, shuffle): reload if the panel is showing.
pub fn on_player_changed(app: &Halo) -> Task<Message> {
    if app.queue_panel.open { reload(app) } else { Task::none() }
}

pub fn subscription(app: &Halo) -> Option<Subscription<Message>> {
    let st = &app.queue_panel;
    let mut subs = vec![];
    if animating(st) || st.anim.is_some() {
        subs.push(iced::time::every(Duration::from_millis(16)).map(|_| qm(Msg::Tick)));
    }
    if st.drag.is_some() {
        subs.push(event::listen_with(|e, _, _| match e {
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(qm(Msg::DragEnd)),
            _ => None,
        }));
    }
    (!subs.is_empty()).then(|| Subscription::batch(subs))
}

pub fn visible(app: &Halo) -> bool {
    app.queue_panel.open || animating(&app.queue_panel)
}

/// The right-edge layer (over the page). Slides by revealing the panel's
/// left part in a clipped box that grows from the window's right edge.
pub fn view(app: &Halo) -> Element<'_, Message> {
    let shown = WIDTH * progress(&app.queue_panel);
    let panel = container(panel(app)).width(WIDTH).height(Length::Fill);
    row![
        Space::new().width(Length::Fill),
        container(panel).width(shown).height(Length::Fill).clip(true),
    ]
    .height(Length::Fill)
    .into()
}

fn panel(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.queue_panel;
    let n = st.data.upcoming.len();

    let clear = button(text("Clear").size(theme::BODY))
        .padding(Padding::from([4.0, 8.0]))
        .style(theme::btn_text(k.muted_fg, k.foreground))
        .on_press_maybe((n > 0).then_some(qm(Msg::Clear)));
    let close = tip(
        button(icon_box(Icon::X, 16.0, 32.0, 32.0))
            .padding(0)
            .style(theme::btn_mode(false))
            .on_press(qm(Msg::Close)),
        "Close (Esc)",
        Position::Left,
    );
    let header = row![
        column![
            text("Up next").size(theme::TITLE).font(theme::SEMIBOLD),
            ui::muted(
                if st.data.shuffled {
                    format!("{} · shuffled", plural(n as i64, "track", "tracks"))
                } else {
                    plural(n as i64, "track", "tracks")
                },
                theme::SMALL
            ),
        ]
        .spacing(2),
        ui::fill(),
        clear,
        close,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let now: Element<'_, Message> = match &st.data.current {
        Some(q) => {
            let playing = app.is_playing();
            column![
                ui::muted(if playing { "Now playing" } else { "Paused" }, theme::SMALL),
                row![
                    art(q.album_art_path.as_deref(), 48.0, theme::R_SM),
                    column![
                        line(title_of(q)).size(theme::BODY).font(theme::MEDIUM).color(k.primary).width(Length::Fill),
                        line(artists_of(q)).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    icon(if playing { Icon::Pause } else { Icon::Play }, 14.0).color(k.primary),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            ]
            .spacing(8)
            .into()
        }
        None => Space::new().height(0).into(),
    };

    let list: Element<'_, Message> = if let Some(e) = &st.error {
        container(text(e.as_str()).size(theme::SMALL).color(k.destructive)).padding(8).into()
    } else if st.loaded && n == 0 {
        container(ui::muted("Nothing up next. Add songs from their row's + button.", theme::BODY))
            .padding(Padding::from([16.0, 0.0]))
            .into()
    } else {
        let drag = st.drag;
        let over = st.over;
        crate::pages::common::vlist(SCROLL_KEY.into(), &app.pages.scroll, n, ROW_H, 8.0, move |i| {
            up_next_row(&st.data.upcoming[i], i, drag, over)
        })
    };

    let mut col = column![header].spacing(20).height(Length::Fill);
    if st.data.current.is_some() {
        col = col.push(now);
    }
    if n > 0 {
        col = col.push(ui::muted("Up next", theme::SMALL));
    }
    col = col.push(list);

    container(col)
        .padding(Padding::from([16.0, 16.0]).bottom(0.0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::side_panel)
        .into()
}

fn title_of(q: &QueueTrack) -> String {
    q.title
        .clone()
        .unwrap_or_else(|| q.file_path.rsplit(['/', '\\']).next().unwrap_or(&q.file_path).to_string())
}

fn artists_of(q: &QueueTrack) -> String {
    if q.artists.is_empty() { "Unknown artist".into() } else { q.artists.join(", ") }
}

fn up_next_row(q: &QueueTrack, i: usize, drag: Option<usize>, over: Option<usize>) -> Element<'_, Message> {
    let k = t();
    let dragged = drag == Some(i);
    let drop_here = drag.is_some() && over == Some(i) && drag != Some(i);
    let grip = mouse_area(center(icon(Icon::GripVertical, 14.0).color(a(k.muted_fg, 0.6))).width(20).height(40))
        .on_press(qm(Msg::DragStart(i)))
        .interaction(mouse::Interaction::Grab);
    let remove = tip(
        button(icon_box(Icon::X, 14.0, 28.0, 28.0))
            .padding(0)
            .style(theme::btn_icon)
            .on_press(qm(Msg::Remove(q.queue_id))),
        "Remove",
        Position::Left,
    );
    let content = row![
        grip,
        art(q.album_art_path.as_deref(), 40.0, theme::R_SM),
        column![
            line(title_of(q)).size(theme::BODY).width(Length::Fill),
            line(artists_of(q)).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
        ]
        .spacing(2)
        .width(Length::Fill),
        iced::widget::hover(Space::new().width(28).height(28), remove),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // Drop line above the row the dragged one will take the place of.
    let line_el = container(Space::new())
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: drop_here.then(|| t().primary.into()),
            ..container::Style::default()
        });
    column![
        line_el,
        surface(content)
            .width(Length::Fill)
            .height(ROW_H - 2.0)
            .padding(Padding::from([0.0, 4.0]))
            .opacity(if dragged { 0.4 } else { 1.0 })
            .on_enter(qm(Msg::DragOver(i)))
            .on_double_click(Message::Player(PlayerCmd::PlayQueueIndex(q.position)))
            .style(move |s| container::Style {
                background: s.hovered.then(|| a(t().muted, 0.35).into()),
                border: theme::no_border(theme::R_MD),
                ..container::Style::default()
            }),
    ]
    .into()
}
