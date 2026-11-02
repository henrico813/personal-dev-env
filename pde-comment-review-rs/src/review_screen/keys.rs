//! This module turns terminal events into the screen's keys. A tap is never
//! a key, and a swipe only scrolls, so a stray tap never answers a comment.
//! Read the `read_key(event)` and `is_waiting_quit(event)` functions to
//! understand the module.

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind};

/// One key the screen acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Char(char),
    Enter,
    Backspace,
    /// Comes from the up arrow or a wheel turn, which a phone sends for a swipe.
    ScrollUp,
    ScrollDown,
    /// Comes from Ctrl-C, which quits even while the user types a note.
    Interrupt,
}

/// Returns the key for a terminal event, or `None` for a tap, a resize or any
/// other event that must not act as a key.
pub(super) fn read_key(event: &Event) -> Option<Key> {
    match event {
        Event::Key(key) => read_key_press(key),
        Event::Mouse(mouse) => match mouse.kind {
            MouseEventKind::ScrollUp => Some(Key::ScrollUp),
            MouseEventKind::ScrollDown => Some(Key::ScrollDown),
            _ => None,
        },
        _ => None,
    }
}

/// Returns true when an event that was waiting before a new request showed
/// asks to quit. Every other waiting event is dropped, so a second tap cannot
/// answer a request the user has not seen.
pub(super) fn is_waiting_quit(event: &Event) -> bool {
    read_key(event) == Some(Key::Interrupt)
}

/// Returns the key for a key press, or `None` for a release.
fn read_key_press(key: &KeyEvent) -> Option<Key> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('c') if control => Some(Key::Interrupt),
        // Other Ctrl and Alt keys would otherwise act as their plain letter.
        _ if control || key.modifiers.contains(KeyModifiers::ALT) => None,
        KeyCode::Char(character) => Some(Key::Char(character)),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Up => Some(Key::ScrollUp),
        KeyCode::Down => Some(Key::ScrollDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{MouseButton, MouseEvent};

    use super::*;

    /// A mouse event of `kind` in the middle of the phone screen.
    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 20,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    }

    /// A Ctrl-C key press.
    fn ctrl_c() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
    }

    /// `read_key` must give no key for a tap or its release.
    /// Otherwise a stray tap on the phone could answer a comment.
    #[test]
    fn read_key_gives_no_key_for_taps() {
        let tap = mouse(MouseEventKind::Down(MouseButton::Left));
        let tap_release = mouse(MouseEventKind::Up(MouseButton::Left));

        assert_eq!(read_key(&tap), None);
        assert_eq!(read_key(&tap_release), None);
    }

    /// `is_waiting_quit` must be true only for Ctrl-C, which reads as a quit
    /// and never as `c`. Otherwise Ctrl-C pressed during a slow answer check
    /// is dropped, or cuts a comment.
    #[test]
    fn is_waiting_quit_only_for_ctrl_c() {
        let answer_key = Event::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        let tap = mouse(MouseEventKind::Down(MouseButton::Left));

        assert!(!is_waiting_quit(&answer_key));
        assert!(!is_waiting_quit(&tap));
        assert!(is_waiting_quit(&ctrl_c()));
    }
}
