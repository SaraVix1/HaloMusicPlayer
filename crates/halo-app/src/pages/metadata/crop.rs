//! Metadata editor — crop view: the chosen image fitted into 540×360, a
//! 55% black veil outside the crop box, a white/80% box border with eight
//! 10px handles; drag to move, drag edges/corners to resize (min 30px).
//! Starts as the largest centred square. "Apply crop" / "Use full image"
//! run `process_art` (resize + compress) and return to the form.

use halo_backend::commands::metadata_editor as me;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::widget::{button, canvas as canvas_widget, center, column, container, image, row, stack, text};
use iced::{mouse, Alignment, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Task, Theme};

use super::{editor, mm, ArtMode, Msg as MMsg};
use crate::app::{bg, Halo, Message};
use crate::theme::{self, a, t};
use crate::ui::{self, spin, spin_angle, Spinner};

const MAX_W: f32 = 540.0;
const MAX_H: f32 = 360.0;
const MIN_CROP: f32 = 30.0;
const HS: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub struct CropState {
    pub raw_path: String,
    pub handle: image::Handle,
    pub natural: Option<(u32, u32)>,
    pub disp: Size,
    pub crop: Rect,
    pub processing: bool,
}

impl CropState {
    pub fn new(path: String) -> Self {
        let natural = ::image::image_dimensions(&path).ok();
        let (disp, crop) = match natural {
            Some((nw, nh)) => {
                let scale = (MAX_W / nw as f32).min(MAX_H / nh as f32).min(1.0);
                let (dw, dh) = ((nw as f32 * scale).round(), (nh as f32 * scale).round());
                let side = dw.min(dh);
                (
                    Size::new(dw, dh),
                    Rect {
                        x: ((dw - side) / 2.0).round(),
                        y: ((dh - side) / 2.0).round(),
                        w: side,
                        h: side,
                    },
                )
            }
            None => (Size::new(MAX_W, 200.0), Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }),
        };
        Self {
            handle: image::Handle::from_path(&path),
            raw_path: path,
            natural,
            disp,
            crop,
            processing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Msg {
    Set(Rect),
    Apply,
    UseFull,
    Processed(Result<String, String>),
}

fn cm(m: Msg) -> Message {
    mm(MMsg::Crop(m))
}

pub fn update(app: &mut Halo, m: Msg) -> Task<Message> {
    let backend = app.backend.clone();
    let Some(ed) = editor(app) else { return Task::none() };
    match m {
        Msg::Set(r) => {
            if let Some(c) = ed.crop.as_mut() {
                c.crop = r;
            }
            Task::none()
        }
        Msg::Apply | Msg::UseFull => {
            let Some(c) = ed.crop.as_mut() else { return Task::none() };
            if c.processing {
                return Task::none();
            }
            let (x, y, w, h) = match (&m, c.natural) {
                (Msg::Apply, Some((nw, nh))) => {
                    let sx = nw as f32 / c.disp.width;
                    let sy = nh as f32 / c.disp.height;
                    (
                        (c.crop.x * sx).round() as u32,
                        (c.crop.y * sy).round() as u32,
                        (c.crop.w * sx).round() as u32,
                        (c.crop.h * sy).round() as u32,
                    )
                }
                _ => (0, 0, 0, 0),
            };
            c.processing = true;
            let src = c.raw_path.clone();
            bg(
                &backend,
                move |b| me::process_art(src, x, y, w, h, b),
                |r| cm(Msg::Processed(r)),
            )
        }
        Msg::Processed(r) => {
            match r {
                Ok(p) => {
                    if let Some(d) = ed.draft.as_mut() {
                        d.new_art_path = Some(p);
                    }
                }
                Err(e) => ed.error = Some(e),
            }
            ed.crop = None;
            ed.mode = ArtMode::Display;
            Task::none()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Grip {
    Move,
    Edge(bool, bool, bool, bool), // n, e, s, w
}

#[derive(Default)]
pub struct DragState {
    drag: Option<(Grip, Point, Rect)>,
}

struct Program {
    crop: Rect,
    disp: Size,
}

fn handles(r: Rect) -> [(Grip, Point); 8] {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let (x2, y2) = (r.x + r.w, r.y + r.h);
    [
        (Grip::Edge(true, false, false, true), Point::new(r.x, r.y)),
        (Grip::Edge(true, false, false, false), Point::new(cx, r.y)),
        (Grip::Edge(true, true, false, false), Point::new(x2, r.y)),
        (Grip::Edge(false, true, false, false), Point::new(x2, cy)),
        (Grip::Edge(false, true, true, false), Point::new(x2, y2)),
        (Grip::Edge(false, false, true, false), Point::new(cx, y2)),
        (Grip::Edge(false, false, true, true), Point::new(r.x, y2)),
        (Grip::Edge(false, false, false, true), Point::new(r.x, cy)),
    ]
}

impl Program {
    fn hit(&self, p: Point) -> Option<Grip> {
        for (g, c) in handles(self.crop) {
            if (p.x - c.x).abs() <= HS / 2.0 + 2.0 && (p.y - c.y).abs() <= HS / 2.0 + 2.0 {
                return Some(g);
            }
        }
        let r = self.crop;
        (p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h).then_some(Grip::Move)
    }
}

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi.max(lo))
}

impl canvas::Program<Message> for Program {
    type State = DragState;

    fn update(
        &self,
        state: &mut DragState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let p = cursor.position_in(bounds)?;
                let g = self.hit(p)?;
                state.drag = Some((g, p, self.crop));
                Some(canvas::Action::capture())
            }
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (g, start, sc) = state.drag?;
                let p = cursor.position_from(bounds.position())?;
                let (dx, dy) = (p.x - start.x, p.y - start.y);
                let (dw, dh) = (self.disp.width, self.disp.height);
                let Rect { mut x, mut y, mut w, mut h } = sc;
                match g {
                    Grip::Move => {
                        x = clamp(x + dx, 0.0, dw - w);
                        y = clamp(y + dy, 0.0, dh - h);
                    }
                    Grip::Edge(n, e, s, wst) => {
                        if e {
                            w = clamp(w + dx, MIN_CROP, dw - x);
                        }
                        if s {
                            h = clamp(h + dy, MIN_CROP, dh - y);
                        }
                        if wst {
                            let nx = clamp(x + dx, 0.0, x + w - MIN_CROP);
                            w += x - nx;
                            x = nx;
                        }
                        if n {
                            let ny = clamp(y + dy, 0.0, y + h - MIN_CROP);
                            h += y - ny;
                            y = ny;
                        }
                    }
                }
                Some(canvas::Action::publish(cm(Msg::Set(Rect { x, y, w, h }))).and_capture())
            }
            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag.take().map(|_| canvas::Action::capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &DragState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let r = self.crop;
        let veil = a(Color::BLACK, 0.55);
        let (w, h) = (bounds.width, bounds.height);
        f.fill_rectangle(Point::ORIGIN, Size::new(w, r.y), veil);
        f.fill_rectangle(Point::new(0.0, r.y + r.h), Size::new(w, h - r.y - r.h), veil);
        f.fill_rectangle(Point::new(0.0, r.y), Size::new(r.x, r.h), veil);
        f.fill_rectangle(Point::new(r.x + r.w, r.y), Size::new(w - r.x - r.w, r.h), veil);
        f.stroke(
            &Path::rectangle(Point::new(r.x, r.y), Size::new(r.w, r.h)),
            Stroke::default().with_width(1.0).with_color(a(Color::WHITE, 0.8)),
        );
        for (_, c) in handles(r) {
            f.fill_rectangle(Point::new(c.x - HS / 2.0, c.y - HS / 2.0), Size::new(HS, HS), Color::WHITE);
        }
        vec![f.into_geometry()]
    }

    fn mouse_interaction(&self, state: &DragState, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        let g = state
            .drag
            .map(|d| d.0)
            .or_else(|| cursor.position_in(bounds).and_then(|p| self.hit(p)));
        match g {
            Some(Grip::Move) => mouse::Interaction::Grab,
            Some(Grip::Edge(n, e, s, w)) => match (n || s, e || w) {
                (true, true) if (n && w) || (s && e) => mouse::Interaction::ResizingDiagonallyDown,
                (true, true) => mouse::Interaction::ResizingDiagonallyUp,
                (true, false) => mouse::Interaction::ResizingVertically,
                _ => mouse::Interaction::ResizingHorizontally,
            },
            None => mouse::Interaction::default(),
        }
    }
}

pub fn view(c: &CropState) -> Element<'_, Message> {
    let k = t();
    let (dw, dh) = (c.disp.width, c.disp.height);
    let mut area = stack![];
    if c.natural.is_some() {
        area = area
            .push(image(c.handle.clone()).width(dw).height(dh))
            .push(canvas_widget(Program { crop: c.crop, disp: c.disp }).width(dw).height(dh));
    } else {
        area = area.push(
            center(text("Failed to load image").size(theme::BODY).color(k.destructive))
                .width(dw)
                .height(dh),
        );
    }
    if c.processing {
        area = area.push(
            center(spin(Spinner::Loader, 28.0, spin_angle(), theme::WHITE))
                .width(dw)
                .height(dh)
                .style(theme::backdrop(0.6)),
        );
    }
    let area = container(area)
        .width(dw)
        .height(dh)
        .clip(true)
        .style(|_| iced::widget::container::Style {
            background: Some(Color::BLACK.into()),
            border: theme::no_border(theme::R_XS),
            ..Default::default()
        });

    let btn = |label: &'static str, primary: bool, msg: Option<Message>| {
        let mut r = row![].spacing(6).align_y(Alignment::Center);
        if primary && c.processing {
            r = r.push(spin(Spinner::Loader, 13.0, spin_angle(), k.primary_fg));
        }
        r = r.push(text(label).size(theme::BODY));
        button(r)
            .padding(Padding::from([6.0, 16.0]))
            .style(move |th, s| {
                let mut st = if primary { theme::btn_primary(th, s) } else { theme::btn_outline(th, s) };
                st.border.radius = theme::radius(theme::R_LG);
                st
            })
            .on_press_maybe(msg)
    };
    let ready = c.natural.is_some() && !c.processing;
    column![
        container(ui::muted("Drag the crop box · resize from edges or corners", theme::SMALL))
            .width(Length::Fill),
        area,
        row![
            btn("Apply crop", true, ready.then(|| cm(Msg::Apply))),
            btn("Use full image", false, (!c.processing).then(|| cm(Msg::UseFull))),
        ]
        .spacing(8),
    ]
    .spacing(12)
    .align_x(Alignment::Center)
    .width(Length::Fill)
    .into()
}
