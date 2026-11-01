//! Turns terminal events into the screen's keys. Taps and other mouse input
//! become no key, so a stray tap never answers a comment.

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

    fn mouse(kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column: 20,
            row: 5,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn ctrl_c() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
    }

    /// Without it, a stray tap on the phone could answer a comment, a swipe
    /// could scroll the wrong way, and Ctrl-C could cut a comment.
    #[test]
    fn taps_ignored_swipes_scroll_ctrl_c_interrupts() {
        let tap = mouse(MouseEventKind::Down(MouseButton::Left));
        let tap_release = mouse(MouseEventKind::Up(MouseButton::Left));
        let swipe_up = mouse(MouseEventKind::ScrollUp);
        let swipe_down = mouse(MouseEventKind::ScrollDown);

        assert_eq!(read_key(&tap), None);
        assert_eq!(read_key(&tap_release), None);
        assert_eq!(read_key(&swipe_up), Some(Key::ScrollUp));
        assert_eq!(read_key(&swipe_down), Some(Key::ScrollDown));
        assert_eq!(read_key(&ctrl_c()), Some(Key::Interrupt));
    }

    /// Without it, Ctrl-C pressed during a slow answer check is dropped with
    /// the waiting keys, and the screen stays open.
    #[test]
    fn waiting_keys_dropped_except_ctrl_c() {
        let answer_key = Event::Key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        let tap = mouse(MouseEventKind::Down(MouseButton::Left));

        assert!(!is_waiting_quit(&answer_key));
        assert!(!is_waiting_quit(&tap));
        assert!(is_waiting_quit(&ctrl_c()));
    }
}
