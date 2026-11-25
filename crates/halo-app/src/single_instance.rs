//! Single-instance guard (replaces `tauri-plugin-single-instance`).
//!
//! The first process owns a per-user local socket (Linux: abstract socket,
//! Windows: named pipe) named after the bundle identifier. Later launches
//! connect, send their argv + cwd as one JSON line and exit; the first
//! process feeds that to `Backend::handle_forwarded_args` (play the files, or
//! raise the window).
//!
//! The name is claimed **before** `Backend::start` (DB, audio device), so
//! launches that race a starting Halo — Explorer starts one process per
//! selected file, all at once — find the name taken and wait for it instead
//! of each starting a full copy. Messages that arrive before the backend is
//! ready are held in an [`Inbox`] and delivered by [`Primary::attach`].

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use halo_backend::{Backend, APP_IDENTIFIER};
use interprocess::local_socket::{
    prelude::*, GenericNamespaced, Listener, ListenerNonblockingMode, ListenerOptions, Name,
    Stream,
};
use serde::{Deserialize, Serialize};

/// How long a second launch keeps trying to reach a Halo that is still
/// starting (owns the name but isn't accepting yet) before giving up.
const WAIT_FOR_PRIMARY: Duration = Duration::from_secs(5);
const RETRY_EVERY: Duration = Duration::from_millis(50);

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Forward {
    argv: Vec<String>,
    cwd: PathBuf,
}

/// Outcome of [`claim`].
pub enum Claim {
    /// This process is Halo: call [`Primary::attach`] once the backend runs.
    Primary(Primary),
    /// A running Halo took our argv; this process should exit.
    Forwarded,
    /// Couldn't tell (socket errors): run without the single-instance guard.
    Unguarded,
}

/// Owner of the socket name. Accepts from the moment it exists; forwarded
/// launches queue up until [`attach`](Primary::attach).
pub struct Primary {
    inbox: Arc<Inbox>,
}

impl Primary {
    /// The backend is up: deliver queued launches and handle later ones
    /// directly.
    pub fn attach(self, backend: Backend) {
        self.inbox
            .attach(Box::new(move |msg: Forward| backend.handle_forwarded_args(&msg.argv, &msg.cwd)));
    }
}

/// Become the running Halo, or hand our argv to the one that already is.
/// Call before starting the backend, and before any window exists.
pub fn claim() -> Claim {
    match name() {
        Ok(name) => claim_name(&name),
        Err(e) => {
            eprintln!("single-instance: bad socket name: {e}");
            Claim::Unguarded
        }
    }
}

fn claim_name(name: &Name<'_>) -> Claim {
    let deadline = Instant::now() + WAIT_FOR_PRIMARY;
    loop {
        if forward(name) {
            return Claim::Forwarded;
        }
        match create_listener(name) {
            Ok(listener) => return Claim::Primary(spawn_acceptor(listener)),
            // Another process owns the name but didn't accept our connection
            // yet (still starting, or busy with another launch): retry.
            Err(e) if e.kind() == ErrorKind::AddrInUse && Instant::now() < deadline => {
                std::thread::sleep(RETRY_EVERY);
            }
            Err(e) => {
                eprintln!("single-instance: can't claim {name:?}: {e}");
                return Claim::Unguarded;
            }
        }
    }
}

/// Per-user socket name. Abstract sockets (Linux) and `\\.\pipe\` names
/// (Windows) are shared by every user on the machine, so without the user
/// part a second user's launch would be forwarded to the first user's Halo.
fn name() -> std::io::Result<Name<'static>> {
    socket_name(&user_tag()).to_ns_name::<GenericNamespaced>()
}

fn socket_name(user: &str) -> String {
    format!("{APP_IDENTIFIER}.{user}.sock")
}

#[cfg(unix)]
fn user_tag() -> String {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }.to_string()
}

#[cfg(windows)]
fn user_tag() -> String {
    sanitize_user(&std::env::var("USERNAME").unwrap_or_default())
}

#[cfg(not(any(unix, windows)))]
fn user_tag() -> String {
    String::from("user")
}

/// Keep a user name safe for a pipe name: ASCII alphanumerics, `-`, `_`;
/// anything else becomes its hex code so distinct names stay distinct.
#[cfg_attr(not(windows), allow(dead_code))]
fn sanitize_user(user: &str) -> String {
    let mut out = String::new();
    for c in user.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c);
        } else {
            out.push_str(&format!("x{:x}", c as u32));
        }
    }
    if out.is_empty() {
        out.push_str("user");
    }
    out
}

/// Try to hand our argv to a running Halo. `true` if it was delivered.
fn forward(name: &Name<'_>) -> bool {
    let Ok(mut stream) = Stream::connect(name.borrow()) else {
        return false;
    };
    let msg = Forward {
        argv: std::env::args().collect(),
        cwd: std::env::current_dir().unwrap_or_default(),
    };
    let Ok(mut line) = serde_json::to_string(&msg) else {
        return false;
    };
    line.push('\n');
    // Windows only lets the process the user just launched bring a window
    // to the front. Pass that right on, so the running Halo can raise itself
    // instead of only flashing its taskbar button.
    #[cfg(windows)]
    allow_foreground();
    stream.write_all(line.as_bytes()).is_ok() && stream.flush().is_ok()
}

#[cfg(windows)]
fn allow_foreground() {
    use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    // SAFETY: plain Win32 call with a constant argument.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

fn create_listener(name: &Name<'_>) -> std::io::Result<Listener> {
    ListenerOptions::new()
        .name(name.borrow())
        .nonblocking(ListenerNonblockingMode::Neither)
        // Unix: replace a leftover socket file (macOS; Linux abstract
        // sockets can't be stale). Never displaces a live listener —
        // `try_overwrite` stays off, so a running Halo yields AddrInUse.
        .reclaim_name(true)
        .create_sync()
}

/// Accept forwarded launches for the rest of the process's life. Each
/// connection is read on its own thread, so a client that connects and
/// never writes can't block the others.
fn spawn_acceptor(listener: Listener) -> Primary {
    let inbox = Arc::new(Inbox::default());
    let rx = Arc::clone(&inbox);
    let spawned = std::thread::Builder::new()
        .name("halo-single-instance".into())
        .spawn(move || {
            for conn in listener.incoming().flatten() {
                let rx = Arc::clone(&rx);
                let _ = std::thread::Builder::new()
                    .name("halo-forwarded".into())
                    .spawn(move || {
                        let mut line = String::new();
                        if BufReader::new(conn).read_line(&mut line).is_ok() {
                            match serde_json::from_str::<Forward>(&line) {
                                Ok(msg) => rx.deliver(msg),
                                Err(e) => eprintln!("single-instance: bad message: {e}"),
                            }
                        }
                    });
            }
        });
    if let Err(e) = spawned {
        eprintln!("single-instance: acceptor thread failed: {e}");
    }
    Primary { inbox }
}

type Handler = Box<dyn Fn(Forward) + Send>;

/// Forwarded launches, held until a handler is attached. Messages are
/// handled under the lock, so queued ones keep their order relative to
/// those arriving while `attach` drains the queue.
#[derive(Default)]
struct Inbox(Mutex<InboxState>);

#[derive(Default)]
struct InboxState {
    pending: Vec<Forward>,
    handler: Option<Handler>,
}

impl Inbox {
    fn deliver(&self, msg: Forward) {
        let mut st = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match &st.handler {
            Some(handle) => handle(msg),
            None => st.pending.push(msg),
        }
    }

    fn attach(&self, handler: Handler) {
        let mut st = self.0.lock().unwrap_or_else(|e| e.into_inner());
        for msg in std::mem::take(&mut st.pending) {
            handler(msg);
        }
        st.handler = Some(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(arg: &str) -> Forward {
        Forward { argv: vec!["halo".into(), arg.into()], cwd: PathBuf::from("/") }
    }

    fn recorder() -> (Arc<Mutex<Vec<String>>>, Handler) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = Arc::clone(&seen);
        (seen, Box::new(move |m: Forward| s.lock().unwrap().push(m.argv.get(1).cloned().unwrap_or_default())))
    }

    #[test]
    fn inbox_holds_messages_until_attached_then_delivers_in_order() {
        let inbox = Inbox::default();
        inbox.deliver(msg("a.mp3"));
        inbox.deliver(msg("b.mp3"));
        let (seen, handler) = recorder();
        assert!(seen.lock().unwrap().is_empty());
        inbox.attach(handler);
        assert_eq!(*seen.lock().unwrap(), ["a.mp3", "b.mp3"]);
        inbox.deliver(msg("c.mp3"));
        assert_eq!(*seen.lock().unwrap(), ["a.mp3", "b.mp3", "c.mp3"]);
        assert!(inbox.0.lock().unwrap().pending.is_empty());
    }

    /// Launches racing each other with nobody running: exactly one becomes
    /// the primary, every other one forwards, and nothing is lost even
    /// though the primary attaches its handler only afterwards.
    #[test]
    fn racing_launches_elect_one_primary() {
        const N: usize = 8;
        let name = format!("{APP_IDENTIFIER}.test.{}.sock", std::process::id());
        let barrier = Arc::new(std::sync::Barrier::new(N));
        let handles: Vec<_> = (0..N)
            .map(|_| {
                let (name, barrier) = (name.clone(), Arc::clone(&barrier));
                std::thread::spawn(move || {
                    let name = name.to_ns_name::<GenericNamespaced>().unwrap();
                    barrier.wait();
                    claim_name(&name)
                })
            })
            .collect();
        let claims: Vec<Claim> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let mut primaries = Vec::new();
        let mut forwarded = 0;
        for c in claims {
            match c {
                Claim::Primary(p) => primaries.push(p),
                Claim::Forwarded => forwarded += 1,
                Claim::Unguarded => panic!("a launch ran unguarded"),
            }
        }
        assert_eq!(primaries.len(), 1);
        assert_eq!(forwarded, N - 1);

        // Forwarded messages are read on their own threads; wait for them.
        let (seen, handler) = recorder();
        let primary = primaries.pop().unwrap();
        primary.inbox.attach(handler);
        let deadline = Instant::now() + Duration::from_secs(5);
        while seen.lock().unwrap().len() < N - 1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(seen.lock().unwrap().len(), N - 1);
    }

    #[test]
    fn socket_name_is_per_user() {
        assert_ne!(socket_name("1000"), socket_name("1001"));
        assert!(socket_name("1000").starts_with(APP_IDENTIFIER));
    }

    #[test]
    fn sanitize_user_keeps_names_distinct_and_pipe_safe() {
        assert_eq!(sanitize_user("sara"), "sara");
        assert_eq!(sanitize_user("Jo_Ann-2"), "Jo_Ann-2");
        assert_eq!(sanitize_user(""), "user");
        let a = sanitize_user("a b");
        let b = sanitize_user("a\\b");
        assert_ne!(a, b);
        for s in [a, b, sanitize_user("சாரா")] {
            assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{s}");
        }
    }

    #[test]
    fn forward_message_round_trips() {
        let m = msg("/m/பாடல்.mp3");
        let line = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Forward>(&line).unwrap(), m);
    }
}
