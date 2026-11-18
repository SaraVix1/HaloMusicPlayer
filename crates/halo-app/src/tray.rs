//! System tray icon + menu (port of the old `tray.rs`): Play / Pause,
//! Previous, Next, Show Halo, Mini player, Quit; left-click shows the main
//! window.
//!
//! Linux: tray-icon needs a GTK main loop, so the icon lives on its own GTK
//! thread. Windows/macOS: the icon must be created on the main (event loop)
//! thread, which is where iced runs `update`, so [`create`] is called from
//! there after the first window opens.

use std::sync::{Mutex, OnceLock};

use iced::Subscription;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    PlayPause,
    Previous,
    Next,
    Show,
    Mini,
    Quit,
}

static TX: OnceLock<UnboundedSender<TrayAction>> = OnceLock::new();
static RX: Mutex<Option<UnboundedReceiver<TrayAction>>> = Mutex::new(None);

fn channel() -> &'static UnboundedSender<TrayAction> {
    TX.get_or_init(|| {
        let (tx, rx) = unbounded_channel();
        *RX.lock().unwrap() = Some(rx);
        tx
    })
}

const ICON_PNG: &[u8] = include_bytes!("../assets/icons/32x32.png");

fn load_icon() -> Option<tray_icon::Icon> {
    let img = image::load_from_memory(ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn build() -> Result<tray_icon::TrayIcon, String> {
    let play_pause = MenuItem::with_id("tray_play_pause", "Play / Pause", true, None);
    let prev = MenuItem::with_id("tray_prev", "Previous", true, None);
    let next = MenuItem::with_id("tray_next", "Next", true, None);
    let show = MenuItem::with_id("tray_show", "Show Halo", true, None);
    let mini = MenuItem::with_id("tray_mini", "Mini player", true, None);
    let quit = MenuItem::with_id("tray_quit", "Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &play_pause,
        &prev,
        &next,
        &PredefinedMenuItem::separator(),
        &show,
        &mini,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .map_err(|e| e.to_string())?;

    let tx = channel().clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let action = match e.id().as_ref() {
            "tray_play_pause" => TrayAction::PlayPause,
            "tray_prev" => TrayAction::Previous,
            "tray_next" => TrayAction::Next,
            "tray_show" => TrayAction::Show,
            "tray_mini" => TrayAction::Mini,
            "tray_quit" => TrayAction::Quit,
            _ => return,
        };
        let _ = tx.send(action);
    }));
    let tx = channel().clone();
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = e
        {
            let _ = tx.send(TrayAction::Show);
        }
    }));

    let mut builder = TrayIconBuilder::new()
        .with_id("halo-tray")
        .with_tooltip("Halo")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false);
    if let Some(icon) = load_icon() {
        builder = builder.with_icon(icon);
    }
    builder.build().map_err(|e| e.to_string())
}

/// Create the tray icon. Safe to call more than once (no-op afterwards).
pub fn create() {
    static CREATED: OnceLock<()> = OnceLock::new();
    if CREATED.set(()).is_err() {
        return;
    }
    let _ = channel();

    #[cfg(target_os = "linux")]
    {
        std::thread::Builder::new()
            .name("halo-tray".into())
            .spawn(|| {
                if gtk::init().is_err() {
                    eprintln!("tray setup failed: GTK init failed");
                    return;
                }
                match build() {
                    Ok(_tray) => gtk::main(),
                    Err(e) => eprintln!("tray setup failed: {e}"),
                }
            })
            .ok();
    }

    #[cfg(not(target_os = "linux"))]
    {
        match build() {
            // The icon must outlive the app; leak it deliberately.
            Ok(tray) => std::mem::forget(tray),
            Err(e) => eprintln!("tray setup failed: {e}"),
        }
    }
}

/// Tray actions as a subscription.
pub fn events() -> Subscription<TrayAction> {
    Subscription::run(|| {
        // The subscription starts before the tray is created (on the main
        // window's first `Opened`), so make sure the channel exists first —
        // otherwise there is no receiver to take and every tray action is lost.
        let _ = channel();
        let rx = RX.lock().ok().and_then(|mut g| g.take());
        iced::stream::channel(16, async move |mut out| {
            use iced::futures::SinkExt;
            let Some(mut rx) = rx else {
                std::future::pending::<()>().await;
                return;
            };
            while let Some(a) = rx.recv().await {
                if out.send(a).await.is_err() {
                    break;
                }
            }
        })
    })
}
