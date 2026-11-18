//! `Surface` — a container whose style depends on hover / press state and
//! which can react to clicks, double clicks and right clicks without
//! swallowing events meant for interactive children (buttons inside a table
//! row still work).
//!
//! This covers the many `hover:bg-…` rows/cards of the web UI, e.g. a track
//! row that plays on double-click and has hover-only action buttons.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, tree, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::widget::container;
use iced::{mouse, Element, Event, Length, Padding, Rectangle, Size, Vector};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub hovered: bool,
    pub pressed: bool,
}

type StyleFn<'a> = Box<dyn Fn(Status) -> container::Style + 'a>;

pub struct Surface<'a, Message> {
    content: Element<'a, Message>,
    style: StyleFn<'a>,
    padding: Padding,
    width: Length,
    height: Length,
    on_press: Option<Message>,
    on_double_click: Option<Message>,
    on_right_press: Option<Message>,
    on_enter: Option<Message>,
    on_exit: Option<Message>,
    interaction: Option<mouse::Interaction>,
    opacity: f32,
}

pub fn surface<'a, Message>(content: impl Into<Element<'a, Message>>) -> Surface<'a, Message> {
    Surface {
        content: content.into(),
        style: Box::new(|_| container::Style::default()),
        padding: Padding::ZERO,
        width: Length::Shrink,
        height: Length::Shrink,
        on_press: None,
        on_double_click: None,
        on_right_press: None,
        on_enter: None,
        on_exit: None,
        interaction: None,
        opacity: 1.0,
    }
}

impl<'a, Message: Clone> Surface<'a, Message> {
    pub fn style(mut self, f: impl Fn(Status) -> container::Style + 'a) -> Self {
        self.style = Box::new(f);
        self
    }
    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        self.padding = p.into();
        self
    }
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        self.width = w.into();
        self
    }
    pub fn height(mut self, h: impl Into<Length>) -> Self {
        self.height = h.into();
        self
    }
    pub fn on_press(mut self, m: Message) -> Self {
        self.on_press = Some(m);
        self
    }
    pub fn on_press_maybe(mut self, m: Option<Message>) -> Self {
        self.on_press = m;
        self
    }
    pub fn on_double_click(mut self, m: Message) -> Self {
        self.on_double_click = Some(m);
        self
    }
    pub fn on_right_press(mut self, m: Message) -> Self {
        self.on_right_press = Some(m);
        self
    }
    pub fn on_enter(mut self, m: Message) -> Self {
        self.on_enter = Some(m);
        self
    }
    pub fn on_exit(mut self, m: Message) -> Self {
        self.on_exit = Some(m);
        self
    }
    pub fn interaction(mut self, i: mouse::Interaction) -> Self {
        self.interaction = Some(i);
        self
    }
    /// Whole-surface opacity (e.g. a row being dragged at 40%).
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o;
        self
    }
}

#[derive(Default)]
struct State {
    hovered: bool,
    pressed: bool,
    last_click: Option<iced::advanced::mouse::Click>,
}

impl<'a, Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for Surface<'a, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::padded(limits, self.width, self.height, self.padding, |limits| {
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits)
        })
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let over = cursor.is_over(bounds) && cursor.is_over(*viewport);

        if over != state.hovered {
            state.hovered = over;
            if over {
                if let Some(m) = &self.on_enter {
                    shell.publish(m.clone());
                }
            } else {
                state.pressed = false;
                if let Some(m) = &self.on_exit {
                    shell.publish(m.clone());
                }
            }
            shell.request_redraw();
        }

        if shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if over => {
                state.pressed = true;
                if let Some(m) = &self.on_press {
                    shell.publish(m.clone());
                    shell.capture_event();
                }
                if let (Some(m), Some(pos)) = (&self.on_double_click, cursor.position()) {
                    let click = iced::advanced::mouse::Click::new(pos, mouse::Button::Left, state.last_click);
                    if click.kind() == iced::advanced::mouse::click::Kind::Double {
                        shell.publish(m.clone());
                    }
                    state.last_click = Some(click);
                    shell.capture_event();
                }
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.pressed {
                    state.pressed = false;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) if over => {
                if let Some(m) = &self.on_right_press {
                    shell.publish(m.clone());
                    shell.capture_event();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let child = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        );
        if child != mouse::Interaction::None {
            return child;
        }
        match self.interaction {
            Some(i) if cursor.is_over(layout.bounds()) => i,
            _ => mouse::Interaction::None,
        }
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
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        if !bounds.intersects(viewport) {
            return;
        }
        let hovered = cursor.is_over(bounds) && cursor.is_over(*viewport);
        let s = (self.style)(Status {
            hovered,
            pressed: state.pressed && hovered,
        });
        let draw_all = |renderer: &mut iced::Renderer| {
            container::draw_background(renderer, &s, bounds);
            let defaults = renderer::Style {
                text_color: s.text_color.unwrap_or(style.text_color),
            };
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &defaults,
                layout.children().next().unwrap(),
                cursor,
                viewport,
            );
        };
        if self.opacity < 1.0 {
            use iced::advanced::Renderer as _;
            renderer.with_layer(bounds, |r| {
                // iced has no group opacity; approximate by drawing then
                // overlaying the page background.
                draw_all(r);
                r.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    crate::theme::a(crate::theme::t().background, 1.0 - self.opacity),
                );
            });
        } else {
            draw_all(renderer);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: Clone + 'a> From<Surface<'a, Message>> for Element<'a, Message> {
    fn from(s: Surface<'a, Message>) -> Self {
        Element::new(s)
    }
}

#[allow(dead_code)]
fn _assert_widget() {
    let _ = widget::Id::unique();
}
