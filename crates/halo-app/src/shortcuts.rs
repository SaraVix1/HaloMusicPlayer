//! Global playback shortcuts and which key presses reach the app at all.
//!
//! | Key | Action |
//! |---|---|
//! | Space | play / pause |
//! | ← / → | seek back / forward 10 s |
//! | Ctrl + ← / → | previous / next song |
//! | Ctrl + L | lyrics panel |
//! | Ctrl + K | search (handled in `Halo::on_key`) |
//!
//! None of them fire while a text field has the key (typing a space or
//! moving the caret), see [`deliver`].

use iced::keyboard::{key::Named, Key, Modifiers};

pub const SEEK_MS: i64 = 10_000;

/// Rows for Settings → General → Keyboard shortcuts.
pub const LIST: &[(&str, &str)] = &[
    ("Space", "Play / pause"),
    ("← / →", "Seek back / forward 10 seconds"),
    ("Ctrl + ← / →", "Previous / next song"),
    ("Ctrl + K", "Search"),
    ("Ctrl + L", "Lyrics"),
    ("Ctrl + M", "Mini player"),
    ("Esc", "Close menus, panels and dialogs"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    TogglePlay,
    /// Relative seek in milliseconds.
    Seek(i64),
    Previous,
    Next,
    Lyrics,
}

fn cmd(m: Modifiers) -> bool {
    m.command() || m.control()
}

/// The playback shortcut for a key press, if any.
pub fn shortcut(key: &Key, mods: Modifiers) -> Option<Shortcut> {
    match key {
        Key::Named(Named::Space) if !cmd(mods) => Some(Shortcut::TogglePlay),
        Key::Named(Named::ArrowLeft) if cmd(mods) => Some(Shortcut::Previous),
        Key::Named(Named::ArrowRight) if cmd(mods) => Some(Shortcut::Next),
        Key::Named(Named::ArrowLeft) => Some(Shortcut::Seek(-SEEK_MS)),
        Key::Named(Named::ArrowRight) => Some(Shortcut::Seek(SEEK_MS)),
        Key::Character(c) if cmd(mods) && c.eq_ignore_ascii_case("l") => Some(Shortcut::Lyrics),
        _ => None,
    }
}

/// Should a key press reach `Halo::on_key`? `captured`: a widget (a text
/// field) already handled it. Esc always (closes things from inside
/// inputs); Ctrl shortcuts too, except Ctrl+arrows, which move the caret by
/// word in a text field; plain keys only when nothing used them.
pub fn deliver(key: &Key, mods: Modifiers, captured: bool) -> bool {
    let escape = matches!(key, Key::Named(Named::Escape));
    let arrows = matches!(key, Key::Named(Named::ArrowLeft | Named::ArrowRight));
    escape || (cmd(mods) && !arrows) || !captured
}

/// New position for a relative seek, clamped to the track.
pub fn seek_target(position_ms: u64, duration_ms: Option<u64>, delta: i64) -> u64 {
    let p = (position_ms as i64 + delta).max(0) as u64;
    match duration_ms {
        Some(d) if d > 0 => p.min(d.saturating_sub(250)),
        _ => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    #[test]
    fn keys_map_to_shortcuts() {
        let none = Modifiers::empty();
        let ctrl = Modifiers::CTRL;
        assert_eq!(shortcut(&Key::Named(Named::Space), none), Some(Shortcut::TogglePlay));
        assert_eq!(shortcut(&Key::Named(Named::ArrowLeft), none), Some(Shortcut::Seek(-SEEK_MS)));
        assert_eq!(shortcut(&Key::Named(Named::ArrowRight), none), Some(Shortcut::Seek(SEEK_MS)));
        assert_eq!(shortcut(&Key::Named(Named::ArrowLeft), ctrl), Some(Shortcut::Previous));
        assert_eq!(shortcut(&Key::Named(Named::ArrowRight), ctrl), Some(Shortcut::Next));
        assert_eq!(shortcut(&ch("l"), ctrl), Some(Shortcut::Lyrics));
        assert_eq!(shortcut(&ch("L"), ctrl), Some(Shortcut::Lyrics));
        assert_eq!(shortcut(&ch("l"), none), None);
        assert_eq!(shortcut(&ch("k"), ctrl), None); // search: on_key
    }

    #[test]
    fn typing_is_left_alone() {
        let none = Modifiers::empty();
        let ctrl = Modifiers::CTRL;
        // A text field took it: Space / arrows / Ctrl+arrows stay there.
        assert!(!deliver(&Key::Named(Named::Space), none, true));
        assert!(!deliver(&Key::Named(Named::ArrowLeft), none, true));
        assert!(!deliver(&Key::Named(Named::ArrowRight), ctrl, true));
        // Ctrl shortcuts and Esc still work from inside a field.
        assert!(deliver(&ch("k"), ctrl, true));
        assert!(deliver(&ch("l"), ctrl, true));
        assert!(deliver(&Key::Named(Named::Escape), none, true));
        // Nothing focused: everything gets through.
        assert!(deliver(&Key::Named(Named::Space), none, false));
        assert!(deliver(&Key::Named(Named::ArrowLeft), ctrl, false));
    }

    #[test]
    fn seek_is_clamped() {
        assert_eq!(seek_target(5_000, Some(200_000), -SEEK_MS), 0);
        assert_eq!(seek_target(60_000, Some(200_000), SEEK_MS), 70_000);
        assert_eq!(seek_target(195_000, Some(200_000), SEEK_MS), 199_750);
        assert_eq!(seek_target(1_000, None, SEEK_MS), 11_000);
    }
}
