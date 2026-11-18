//! Anchored popover — renders `popup` in an overlay next to `anchor` (like
//! the absolutely positioned popovers / dropdowns of the web UI) and
//! publishes `on_dismiss` on a mouse press outside both.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::{keyboard, mouse, Element, Event, Length, Point, Rectangle, Size, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Placement {
    /// Above the anchor, right edges aligned (`bottom-full right-0`).
    AboveEnd,
    /// Above the anchor, left edges aligned.
    AboveStart,
    /// Below the anchor, left edges aligned (`top-full left-0`).
    BelowStart,
    /// Below the anchor, right edges aligned.
    BelowEnd,
}

pub struct Popover<'a, Message> {
    anchor: Element<'a, Message>,
    popup: Option<Element<'a, Message>>,
    placement: Placement,
    gap: f32,
    on_dismiss: Option<Message>,
}

pub fn popover<'a, Message>(
    anchor: impl Into<Element<'a, Message>>,
    popup: Option<Element<'a, Message>>,
) -> Popover<'a, Message> {
    Popover {
        anchor: anchor.into(),
        popup,
        placement: Placement::AboveEnd,
        gap: 8.0,
        on_dismiss: None,
    }
}

impl<'a, Message> Popover<'a, Message> {
    pub fn placement(mut self, p: Placement) -> Self {
        self.placement = p;
        self
    }
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    pub fn on_dismiss(mut self, m: Message) -> Self {
        self.on_dismiss = Some(m);
        self
    }
}

impl<'a, Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for Popover<'a, Message> {
    fn children(&self) -> Vec<Tree> {
        let mut v = vec![Tree::new(&self.anchor)];
        if let Some(p) = &self.popup {
            v.push(Tree::new(p));
        }
        v
    }

    fn diff(&self, tree: &mut Tree) {
        match &self.popup {
            Some(p) => tree.diff_children(&[&self.anchor, p]),
            None => tree.diff_children(std::slice::from_ref(&self.anchor)),
        }
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.anchor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.anchor
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.anchor
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let (anchor_tree, rest) = tree.children.split_at_mut(1);
        let anchor_overlay = self.anchor.as_widget_mut().overlay(
            &mut anchor_tree[0],
            layout,
            renderer,
            viewport,
            translation,
        );
        let popup = self.popup.as_mut().map(|popup| {
            overlay::Element::new(Box::new(PopupOverlay {
                popup,
                tree: &mut rest[0],
                anchor_bounds: layout.bounds() + translation,
                placement: self.placement,
                gap: self.gap,
                on_dismiss: self.on_dismiss.clone(),
            }))
        });
        if anchor_overlay.is_some() || popup.is_some() {
            Some(
                overlay::Group::with_children(anchor_overlay.into_iter().chain(popup).collect())
                    .overlay(),
            )
        } else {
            None
        }
    }
}

struct PopupOverlay<'a, 'b, Message> {
    popup: &'b mut Element<'a, Message>,
    tree: &'b mut Tree,
    anchor_bounds: Rectangle,
    placement: Placement,
    gap: f32,
    on_dismiss: Option<Message>,
}

impl<Message: Clone> overlay::Overlay<Message, iced::Theme, iced::Renderer>
    for PopupOverlay<'_, '_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        let node = self.popup.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );
        let size = node.size();
        let a = self.anchor_bounds;
        let (mut x, mut y) = match self.placement {
            Placement::AboveEnd => (a.x + a.width - size.width, a.y - self.gap - size.height),
            Placement::AboveStart => (a.x, a.y - self.gap - size.height),
            Placement::BelowStart => (a.x, a.y + a.height + self.gap),
            Placement::BelowEnd => (a.x + a.width - size.width, a.y + a.height + self.gap),
        };
        x = x.clamp(0.0, (bounds.width - size.width).max(0.0));
        y = y.clamp(0.0, (bounds.height - size.height).max(0.0));
        node.move_to(Point::new(x, y))
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.popup.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.popup
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(_))
                if !cursor.is_over(bounds) && !cursor.is_over(self.anchor_bounds) =>
            {
                if let Some(m) = &self.on_dismiss {
                    shell.publish(m.clone());
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                if let Some(m) = &self.on_dismiss {
                    shell.publish(m.clone());
                    shell.capture_event();
                }
            }
            _ => {}
        }
        self.popup.as_widget_mut().update(
            self.tree, event, layout, cursor, renderer, clipboard, shell, &bounds,
        );
        // Swallow clicks inside the popup so widgets underneath don't react.
        if cursor.is_over(bounds)
            && matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_)))
        {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.popup.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'c, Message, iced::Theme, iced::Renderer>> {
        self.popup.as_widget_mut().overlay(
            self.tree,
            layout,
            renderer,
            &layout.bounds(),
            Vector::ZERO,
        )
    }
}

impl<'a, Message: Clone + 'a> From<Popover<'a, Message>> for Element<'a, Message> {
    fn from(p: Popover<'a, Message>) -> Self {
        Element::new(p)
    }
}
