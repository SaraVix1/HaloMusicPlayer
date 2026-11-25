//! Folders view (`folders-view.tsx`): flat list of leaf folders (folders that
//! directly contain tracks) on the left, the selected folder's tracks on the
//! right with Play All / Shuffle.

use std::collections::BTreeMap;

use halo_backend::commands::library::FolderTrack;
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding};

use super::common::{self, note};
use super::library::Coll;
use super::Msg;
use crate::app::{Halo, Message};
use crate::format::{duration, plural};
use crate::icons::{icon, Icon};
use crate::theme::{self, a, t};
use crate::ui::{self, art, hline, vline};
use crate::widgets::{line, surface};

#[derive(Default)]
pub struct FoldersState {
    pub rows: Coll<FolderTrack>,
    pub leaves: Vec<Leaf>,
    pub selected: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Leaf {
    pub path: String,
    pub name: String,
    pub tracks: Vec<FolderTrack>,
}

fn norm(s: &str) -> String {
    s.replace('\\', "/").trim_end_matches('/').to_string()
}

fn relative_segments(file: &str, folder: &str) -> Vec<String> {
    let f = norm(folder);
    let p = norm(file);
    let rest = if p.to_lowercase().starts_with(&(f.to_lowercase() + "/")) {
        p[f.len() + 1..].to_string()
    } else {
        p
    };
    rest.split('/').filter(|s| !s.is_empty()).map(String::from).collect()
}

#[derive(Default)]
struct Node {
    path: String,
    children: BTreeMap<String, Node>,
    tracks: Vec<FolderTrack>,
}

fn collect(nodes: Vec<Node>, out: &mut Vec<Leaf>) {
    for n in nodes {
        if n.children.is_empty() {
            if !n.tracks.is_empty() {
                let name = n
                    .path
                    .rsplit(['/', '\\'])
                    .find(|s| !s.is_empty())
                    .unwrap_or(&n.path)
                    .to_string();
                let mut tracks = n.tracks;
                tracks.sort_by_key(|t| t.file_path.to_lowercase());
                out.push(Leaf {
                    path: n.path,
                    name,
                    tracks,
                });
            }
        } else {
            let mut kids: Vec<(String, Node)> = n.children.into_iter().collect();
            kids.sort_by_key(|(k, _)| k.to_lowercase());
            collect(kids.into_iter().map(|(_, v)| v).collect(), out);
        }
    }
}

/// Build the folder tree from flat rows and return its leaf folders.
pub fn leaves(rows: &[FolderTrack]) -> Vec<Leaf> {
    let mut roots: BTreeMap<i64, Node> = BTreeMap::new();
    let mut order: Vec<i64> = vec![];
    for r in rows {
        let root = roots.entry(r.folder_id).or_insert_with(|| {
            order.push(r.folder_id);
            Node {
                path: r.folder_path.clone(),
                ..Node::default()
            }
        });
        let segs = relative_segments(&r.file_path, &r.folder_path);
        if segs.is_empty() {
            continue;
        }
        let mut node = root;
        let mut path = node.path.clone();
        for seg in &segs[..segs.len() - 1] {
            path = format!("{path}/{seg}");
            let p = path.clone();
            node = node.children.entry(seg.clone()).or_insert_with(|| Node {
                path: p,
                ..Node::default()
            });
        }
        node.tracks.push(r.clone());
    }
    let mut out = vec![];
    collect(order.into_iter().filter_map(|id| roots.remove(&id)).collect(), &mut out);
    out
}

pub fn on_loaded(app: &mut Halo, r: Result<Vec<FolderTrack>, String>) {
    let st = &mut app.pages.folders;
    st.rows.set(r);
    st.leaves = leaves(&st.rows.items);
    let still_there = st
        .selected
        .as_ref()
        .is_some_and(|s| st.leaves.iter().any(|l| &l.path == s));
    if !still_there {
        st.selected = st.leaves.first().map(|l| l.path.clone());
    }
}

pub fn view(app: &Halo) -> Element<'_, Message> {
    let k = t();
    let st = &app.pages.folders;

    // ── Left list ─────────────────────────────────────────────────────────────
    let mut head = row![text("Folders").size(theme::BODY).font(theme::SEMIBOLD), ui::fill()]
        .align_y(Alignment::Center);
    if !st.leaves.is_empty() {
        head = head.push(ui::muted(
            plural(st.leaves.len() as i64, "folder", "folders"),
            theme::SMALL,
        ));
    }
    let head = container(head).padding(Padding::from([12.0, 16.0])).height(45);

    let list: Element<'_, Message> = if st.rows.loading {
        container(ui::muted("Loading…", theme::SMALL)).padding(16).into()
    } else if let Some(e) = &st.rows.error {
        container(text(e.as_str()).size(theme::SMALL).color(k.destructive))
            .padding(16)
            .into()
    } else if st.leaves.is_empty() {
        container(ui::muted("No managed folders yet — add one in Settings.", theme::SMALL))
            .padding(16)
            .into()
    } else {
        let mut col = column![];
        for leaf in &st.leaves {
            let selected = st.selected.as_deref() == Some(&leaf.path);
            let sub_c = if selected { a(k.accent_fg, 0.6) } else { k.muted_fg };
            let content = row![
                icon(Icon::Folder, 15.0).color(if selected { k.accent_fg } else { k.muted_fg }),
                column![
                    line(leaf.name.as_str()).size(theme::BODY).font(theme::MEDIUM).width(Length::Fill),
                    text(plural(leaf.tracks.len() as i64, "track", "tracks"))
                        .size(theme::SMALL)
                        .color(sub_c),
                ]
                .spacing(2)
                .width(Length::Fill),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            col = col.push(
                button(content)
                    .width(Length::Fill)
                    .padding(Padding::from([10.0, 12.0]))
                    .style(theme::btn_list_item(selected, 0.0))
                    .on_press(Message::Page(Msg::SelectFolder(leaf.path.clone()))),
            );
        }
        common::scroll_page("folders:list".into(), col)
    };

    let left = column![head, hline(), list].width(288).height(Length::Fill);

    // ── Right detail ──────────────────────────────────────────────────────────
    let selected = st
        .selected
        .as_ref()
        .and_then(|s| st.leaves.iter().find(|l| &l.path == s));
    let right: Element<'_, Message> = match selected {
        None => iced::widget::center(note("Select a folder to view its tracks.")).into(),
        Some(leaf) => detail(app, leaf),
    };

    row![left, vline(), container(right).width(Length::Fill).height(Length::Fill)]
        .height(Length::Fill)
        .into()
}

fn detail<'a>(app: &'a Halo, leaf: &'a Leaf) -> Element<'a, Message> {
    let k = t();
    let total: i64 = leaf.tracks.iter().map(|t| t.duration_ms.unwrap_or(0)).sum();
    let art_path = leaf.tracks.iter().find_map(|t| t.album_art_path.as_deref());
    let ids: Vec<i64> = leaf.tracks.iter().map(|t| t.id).collect();

    let small_btn = |lead: Element<'a, Message>, label: &'static str, primary: bool, msg: Message| {
        button(
            row![lead, text(label).size(theme::SMALL)]
                .spacing(6)
                .align_y(Alignment::Center),
        )
        .padding(Padding::from([6.0, 12.0]))
        .style(move |th, st| {
            let mut s = if primary {
                theme::btn_primary(th, st)
            } else {
                theme::btn_outline(th, st)
            };
            s.border.radius = theme::radius(theme::R_MD);
            s
        })
        .on_press(msg)
    };

    let header = row![
        art(art_path, 64.0, theme::R_MD),
        column![
            line(leaf.name.as_str()).size(theme::BODY).font(theme::SEMIBOLD).width(Length::Fill),
            text(format!(
                "{} · {}",
                plural(leaf.tracks.len() as i64, "track", "tracks"),
                duration(Some(total))
            ))
            .size(theme::SMALL)
            .color(k.muted_fg),
            ui::vspace(8.0),
            row![
                small_btn(
                    ui::filled(ui::Filled::Play, 11.0, k.primary_fg).into(),
                    "Play All",
                    true,
                    Message::Page(Msg::FolderPlay(ids.clone(), 0, false)),
                ),
                small_btn(
                    icon(Icon::Shuffle, 11.0).into(),
                    "Shuffle",
                    false,
                    Message::Page(Msg::FolderPlay(ids.clone(), 0, true)),
                ),
            ]
            .spacing(8),
        ]
        .spacing(2)
        .width(Length::Fill),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    let header = container(header).padding(Padding::from([16.0, 24.0]));

    let mut list = column![];
    for (i, tr) in leaf.tracks.iter().enumerate() {
        let title = tr.title.clone().unwrap_or_else(|| {
            tr.file_path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&tr.file_path)
                .to_string()
        });
        let content = row![
            container(text((i + 1).to_string()).size(theme::SMALL).color(k.muted_fg))
                .width(20)
                .align_x(Alignment::End),
            icon(Icon::Music, 13.0).color(k.muted_fg),
            line(title).size(theme::BODY).width(Length::Fill),
            text(duration(tr.duration_ms)).size(theme::SMALL).color(k.muted_fg),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        list = list.push(
            surface(content)
                .width(Length::Fill)
                .padding(Padding::from([8.0, 24.0]))
                .on_press(Message::Page(Msg::FolderPlay(ids.clone(), i as i64, false)))
                .interaction(iced::mouse::Interaction::Pointer)
                .style(|s| iced::widget::container::Style {
                    background: s.hovered.then(|| a(t().muted, 0.3).into()),
                    ..Default::default()
                }),
        );
    }
    let _ = app;
    column![
        header,
        hline(),
        common::scroll_page(format!("folders:{}", leaf.path), list),
        Space::new().height(0),
    ]
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ft(id: i64, folder: &str, file: &str) -> FolderTrack {
        FolderTrack {
            id,
            folder_id: 1,
            folder_path: folder.into(),
            file_path: file.into(),
            title: None,
            duration_ms: None,
            album_art_path: None,
        }
    }

    #[test]
    fn leaves_are_sorted_and_nested() {
        let rows = vec![
            ft(1, "/m", "/m/B/x/1.mp3"),
            ft(2, "/m", "/m/a/2.mp3"),
            ft(3, "/m", "/m/B/x/0.mp3"),
        ];
        let l = leaves(&rows);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].name, "a");
        assert_eq!(l[1].name, "x");
        assert_eq!(l[1].tracks[0].id, 3);
    }
}
