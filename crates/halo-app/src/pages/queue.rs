//! Queue page (`queue.tsx`): virtualised 48px rows with play/pause, art,
//! title + "artists · album", duration and a hover-only remove button;
//! header count + Clear.

use halo_backend::commands::player::{self, QueueTrack};
use iced::widget::{button, column, container, hover, row, text};
use iced::{Alignment, Element, Length, Padding, Task};

use super::common::{self, note};
use super::library::Coll;
use super::Msg;
use crate::app::{bg, fire, Halo, Message, PlayerCmd};
use crate::format::duration;
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, art, hline, icon_box};
use crate::widgets::{line, surface};

pub const ROW_H: f32 = 48.0;

#[derive(Default)]
pub struct QueueState {
    pub items: Coll<QueueTrack>,
}

pub fn load(app: &mut Halo) -> Task<Message> {
    app.pages.queue.items.loading = !app.pages.queue.items.loaded;
    bg(
        &app.backend,
        |b| player::get_queue(b.db()),
        |r| Message::Page(Msg::QueueLoaded(r)),
    )
}

pub fn row_click(app: &mut Halo, idx: i64) -> Task<Message> {
    if Some(idx) == app.player.current_index {
        app.update(Message::Player(PlayerCmd::TogglePlay))
    } else {
        app.update(Message::Player(PlayerCmd::PlayQueueIndex(idx)))
    }
}

pub fn remove(app: &mut Halo, queue_id: i64) -> Task<Message> {
    fire(&app.backend, move |b| player::remove_from_queue(b, queue_id))
}

pub fn clear(app: &mut Halo) -> Task<Message> {
    fire(&app.backend, player::clear_queue)
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.queue.items;
    let n = st.items.len();

    let mut right = row![common::count(n, "track", "tracks")]
        .spacing(12)
        .align_y(Alignment::Center);
    if n > 0 {
        right = right.push(
            button(
                row![icon(Icon::Trash2, 12.0), text("Clear").size(theme::SMALL)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding(Padding::from([4.0, 10.0]))
            .style(|th, s| {
                let mut st = theme::btn_outline(th, s);
                st.border.radius = theme::radius(theme::R_MD);
                st
            })
            .on_press(Message::Page(Msg::ClearQueue)),
        );
    }
    let header = common::header("Queue", right);

    let body: Element<'_, Message> = if st.loading {
        note("Loading…")
    } else if n == 0 {
        note("Queue is empty. Play a track from the library to get started.")
    } else {
        let current = app.player.current_index;
        let playing = app.is_playing();
        let list = common::vlist(app.route().scroll_key(), &app.pages.scroll, n, ROW_H, 0.0, move |i| {
            let q = &st.items[i];
            let is_current = Some(i as i64) == current;
            let is_playing = is_current && playing;
            let title = q.title.clone().unwrap_or_else(|| {
                q.file_path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&q.file_path)
                    .to_string()
            });
            let mut sub = if q.artists.is_empty() {
                "—".to_string()
            } else {
                q.artists.join(", ")
            };
            if let Some(al) = &q.album_name {
                sub.push_str(&format!(" · {al}"));
            }
            let base = row![
                button(icon_box(if is_playing { Icon::Pause } else { Icon::Play }, 14.0, 32.0, 32.0))
                    .padding(0)
                    .style(theme::btn_icon)
                    .on_press(Message::Page(Msg::QueueRowClick(i as i64))),
                art(q.album_art_path.as_deref(), 32.0, theme::R_SM),
                column![
                    line(title)
                        .size(theme::BODY)
                        .font(if is_current { theme::MEDIUM } else { theme::SANS })
                        .width(Length::Fill),
                    line(sub).size(theme::SMALL).color(k.muted_fg).width(Length::Fill),
                ]
                .width(Length::Fill),
                container(text(duration(q.duration_ms)).size(theme::SMALL).color(k.muted_fg))
                    .width(48)
                    .align_x(Alignment::End),
                ui::hspace(28.0),
            ]
            .spacing(12)
            .padding(Padding::from([0.0, 12.0]))
            .height(Length::Fill)
            .align_y(Alignment::Center);
            let remove = row![
                ui::fill(),
                button(icon_box(Icon::X, 14.0, 28.0, 28.0))
                    .padding(0)
                    .style(theme::btn_icon)
                    .on_press(Message::Page(Msg::RemoveFromQueue(q.queue_id))),
            ]
            .padding(Padding::ZERO.right(12.0))
            .height(Length::Fill)
            .align_y(Alignment::Center);
            let mut col = column![];
            if i > 0 {
                col = col.push(hline());
            }
            col.push(
                surface(hover(base, remove))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: is_current.then(|| a(t().muted, 0.4).into()),
                        ..Default::default()
                    }),
            )
            .into()
        });
        container(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .clip(true)
            .style(move |_| iced::widget::container::Style {
                border: theme::border(k.border, 1.0, theme::R_MD),
                ..Default::default()
            })
            .into()
    };

    common::padded(column![header, body].spacing(16).height(Length::Fill))
}
