//! `RightClickProbe` — an invisible, full-window top layer that reports
//! where right clicks happen, in window coordinates.
//!
//! Widgets inside a `scrollable` see the cursor in the scrolled content's
//! coordinates (offset by the scroll position), so a row can't tell where
//! on screen it was clicked. This layer sits outside every scrollable, so
//! its cursor is the window's. It draws nothing, never captures events and
//! reports no mouse interaction, so everything below keeps working.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::{mouse, Element, Event, Length, Point, Rectangle, Size};

pub struct RightClickProbe<'a, Message> {
    on_right: Box<dyn Fn(Point) -> Message + 'a>,
}

pub fn right_click_probe<'a, Message>(on_right: impl Fn(Point) -> Message + 'a) -> RightClickProbe<'a, Message> {
    RightClickProbe { on_right: Box::new(on_right) }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for RightClickProbe<'a, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let (Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)), Some(p)) = (event, cursor.position()) {
            // Observe only: the row / card below still gets the click.
            shell.publish((self.on_right)(p));
        }
    }
}

impl<'a, Message: 'a> From<RightClickProbe<'a, Message>> for Element<'a, Message> {
    fn from(p: RightClickProbe<'a, Message>) -> Self {
        Element::new(p)
    }
}
