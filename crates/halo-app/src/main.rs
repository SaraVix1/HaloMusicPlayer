// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use halo_backend::{Backend, BackendEvent};
use iced::widget::{center, column, text};
use iced::{window, Element, Size, Subscription, Task};

fn main() -> iced::Result {
    halo_backend::platform::set_app_user_model_id();
    let backend = match Backend::start() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Halo failed to start: {e}");
            std::process::exit(1);
        }
    };

    iced::daemon(
        move || Halo::new(backend.clone()),
        Halo::update,
        Halo::view,
    )
    .title("Halo")
    .subscription(Halo::subscription)
    .run()
}

struct Halo {
    backend: Backend,
    title: String,
}

#[derive(Debug, Clone)]
enum Message {
    Backend(BackendEvent),
    WindowClosed(window::Id),
}

impl Halo {
    fn new(backend: Backend) -> (Self, Task<Message>) {
        let (_id, open) = window::open(window::Settings {
            size: Size::new(1200.0, 750.0),
            min_size: Some(Size::new(900.0, 600.0)),
            ..window::Settings::default()
        });
        (
            Self {
                backend,
                title: "No track playing".into(),
            },
            open.discard(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Backend(BackendEvent::PlayerState(s)) => {
                self.title = s
                    .current_track
                    .and_then(|t| t.title)
                    .unwrap_or_else(|| "No track playing".into());
                Task::none()
            }
            Message::Backend(_) => Task::none(),
            Message::WindowClosed(_) => iced::exit(),
        }
    }

    fn view(&self, _id: window::Id) -> Element<'_, Message> {
        center(column![text("Halo").size(24), text(&self.title)].spacing(8)).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let backend = self.backend.clone();
        Subscription::batch([
            Subscription::run_with(BackendStream(backend), backend_events).map(Message::Backend),
            window::close_events().map(Message::WindowClosed),
        ])
    }
}

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
        let Some(mut rx) = rx else { return };
        while let Some(ev) = rx.recv().await {
            if out.send(ev).await.is_err() {
                break;
            }
        }
    })
}
