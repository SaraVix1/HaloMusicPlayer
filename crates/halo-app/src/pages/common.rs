//! Shared page building blocks: virtualised list / grid (the
//! `@tanstack/react-virtual` + `VirtualGrid` replacements), per-route scroll
//! memory, and the common page header / empty-state widgets.

use std::collections::HashMap;

use iced::widget::{column, container, responsive, row, scrollable, space, text, Column};
use iced::{Element, Length, Padding, Task};

use crate::app::Message;
use crate::theme::{self, t};
use crate::ui;

/// Scroll offsets per scroll key (survives page switches for the session).
#[derive(Default)]
pub struct ScrollMemory {
    offsets: HashMap<String, f32>,
    viewports: HashMap<String, f32>,
}

impl ScrollMemory {
    pub fn offset(&self, key: &str) -> f32 {
        self.offsets.get(key).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, key: String, offset: f32, viewport: f32) {
        self.viewports.insert(key.clone(), viewport);
        self.offsets.insert(key, offset);
    }

    pub fn viewport(&self, key: &str) -> Option<f32> {
        self.viewports.get(key).copied()
    }

    /// Restore the remembered offset of `key` (or reset to the top).
    pub fn restore<T>(&self, key: &str) -> Task<T> {
        iced::widget::operation::scroll_to(
            iced::widget::Id::from(key.to_string()),
            scrollable::AbsoluteOffset {
                x: None,
                y: Some(self.offset(key)),
            },
        )
    }
}

/// Message constructor for scroll notifications.
pub fn on_scroll(key: String) -> impl Fn(scrollable::Viewport) -> Message {
    move |vp| {
        Message::Page(super::Msg::Scrolled(
            key.clone(),
            vp.absolute_offset().y,
            vp.bounds().height,
        ))
    }
}

/// Visible index window for a uniform-height list.
pub fn window(count: usize, row_h: f32, offset: f32, viewport: f32, overscan: usize) -> (usize, usize) {
    if count == 0 || row_h <= 0.0 {
        return (0, 0);
    }
    let first = ((offset / row_h).floor() as usize).saturating_sub(overscan);
    let last = (((offset + viewport) / row_h).ceil() as usize + overscan).min(count);
    (first.min(count), last.max(first.min(count)))
}

/// Virtualised vertical list of `count` rows, each `row_h` tall. `row` builds
/// the element for an index. `key` identifies the scrollable for scroll
/// memory.
pub fn vlist<'a>(
    key: String,
    memory: &'a ScrollMemory,
    count: usize,
    row_h: f32,
    padding_bottom: f32,
    row: impl Fn(usize) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    let offset = memory.offset(&key);
    responsive(move |size| {
        let viewport = memory.viewport(&key).unwrap_or(size.height);
        let (first, last) = window(count, row_h, offset, viewport, 10);
        let mut col = Column::new().width(Length::Fill);
        col = col.push(space().height(first as f32 * row_h));
        for i in first..last {
            col = col.push(container(row(i)).height(row_h).width(Length::Fill));
        }
        col = col.push(space().height((count - last) as f32 * row_h + padding_bottom));
        scrollable(col)
            .id(iced::widget::Id::from(key.clone()))
            .on_scroll(on_scroll(key.clone()))
            .direction(theme::vscroll())
            .style(theme::scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    })
    .into()
}

/// Grid layout parameters (`VirtualGrid` props).
#[derive(Clone, Copy)]
pub struct GridSpec {
    pub min_item_width: f32,
    pub row_height: f32,
    pub gap: f32,
    pub px: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
}

/// Virtualised responsive grid: `cols = floor((w + gap) / (min + gap))`,
/// equal-width columns, uniform row height, overscan 3 rows.
pub fn vgrid<'a>(
    key: String,
    memory: &'a ScrollMemory,
    count: usize,
    spec: GridSpec,
    item: impl Fn(usize, f32) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    let offset = memory.offset(&key);
    responsive(move |size| {
        // 5px scrollbar lives outside the content area.
        let w = size.width - spec.px * 2.0;
        let cols = (((w + spec.gap) / (spec.min_item_width + spec.gap)).floor() as usize).max(1);
        let rows = count.div_ceil(cols);
        let cell_w = ((w - spec.gap * (cols as f32 - 1.0)) / cols as f32).max(0.0);
        let stride = spec.row_height + spec.gap;
        let viewport = memory.viewport(&key).unwrap_or(size.height);
        let (first, last) = window(
            rows,
            stride,
            (offset - spec.padding_top).max(0.0),
            viewport,
            3,
        );

        let mut col = Column::new().width(Length::Fill);
        col = col.push(space().height(spec.padding_top + first as f32 * stride));
        for r in first..last {
            let mut line = row![].spacing(spec.gap).height(spec.row_height);
            for c in 0..cols {
                let idx = r * cols + c;
                if idx < count {
                    line = line.push(
                        container(item(idx, cell_w))
                            .width(Length::FillPortion(1))
                            .height(Length::Fill),
                    );
                } else {
                    line = line.push(space().width(Length::FillPortion(1)));
                }
            }
            col = col.push(container(line).padding(Padding::from([0.0, spec.px])));
            if r + 1 < rows {
                col = col.push(space().height(spec.gap));
            }
        }
        let rest = rows.saturating_sub(last) as f32 * stride;
        col = col.push(space().height(rest + spec.padding_bottom));

        scrollable(col)
            .id(iced::widget::Id::from(key.clone()))
            .on_scroll(on_scroll(key.clone()))
            .direction(theme::vscroll())
            .style(theme::scroll)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    })
    .into()
}

/// Plain (non-virtualised) scrollable page body with scroll memory.
pub fn scroll_page<'a>(key: String, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    scrollable(content)
        .id(iced::widget::Id::from(key.clone()))
        .on_scroll(on_scroll(key))
        .direction(theme::vscroll())
        .style(theme::scroll)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// `text-sm text-muted-foreground` paragraph (loading / empty states).
pub fn note<'a>(s: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    ui::muted(s, theme::BODY).into()
}

pub fn error_text<'a>(e: &'a str) -> Element<'a, Message> {
    text(e).size(theme::BODY).color(t().destructive).into()
}

/// Page header row: h1 on the left, `right` content on the right
/// (`flex items-baseline justify-between`).
pub fn header<'a>(title: &'a str, right: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![ui::h1(title), ui::fill(), right.into()]
        .align_y(iced::Alignment::Center)
        .spacing(16)
        .into()
}

/// "N item(s)" count label (`text-sm text-muted-foreground tabular-nums`).
pub fn count<'a>(n: usize, one: &str, many: &str) -> Element<'a, Message> {
    ui::muted(crate::format::plural(n as i64, one, many), theme::BODY).into()
}

/// A vertical stack with standard page padding (`p-6`).
pub fn padded<'a>(content: Column<'a, Message>) -> Element<'a, Message> {
    container(content)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[allow(dead_code)]
pub fn spacer<'a>(h: f32) -> Element<'a, Message> {
    column![].height(h).into()
}

#[cfg(test)]
mod tests {
    use super::window;

    #[test]
    fn window_bounds() {
        assert_eq!(window(0, 44.0, 0.0, 500.0, 10), (0, 0));
        assert_eq!(window(100, 44.0, 0.0, 440.0, 10), (0, 20));
        assert_eq!(window(100, 44.0, 4400.0, 440.0, 10), (90, 100));
        assert_eq!(window(5, 44.0, 0.0, 1000.0, 10), (0, 5));
    }
}
