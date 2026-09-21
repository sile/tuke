//! Inputs the edge feeds into the Sans I/O core.

use termnix::{KeyCode as GuestKeyCode, KeyEvent, Modifiers};

/// An event delivered to [`State::update`](crate::state::State::update).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The terminal was resized to `size`.
    Resize {
        /// The new terminal size, in cells.
        size: tuinix::Size,
    },

    /// A left-button release happened at `position`, in screen coordinates
    /// (zero-based, measured from the terminal's top-left corner).
    ///
    /// The core translates this into the layout's coordinate system using the
    /// current keyboard offset before hit-testing.
    PointerRelease {
        /// Where the release happened.
        position: tuinix::Position,
    },

    /// A key typed on the host keyboard was pressed.
    ///
    /// Soft-keyboard keys are delivered as [`Event::PointerRelease`]. This is
    /// the host's own keyboard, and its keys are forwarded to the child
    /// unchanged: tuke reserves no key of its own, so the child sees every
    /// press (including `q` and `C-c`).
    Key {
        /// The pressed key.
        code: tuinix::KeyCode,
        /// Whether Ctrl was held.
        ctrl: bool,
        /// Whether Alt was held.
        alt: bool,
    },
}

impl Event {
    /// Converts a host key event into the guest key event to forward.
    ///
    /// The host key codes and the guest key codes are different types because
    /// they come from different toolkits, so this is the one place where a
    /// press on the host keyboard becomes something the child can be given.
    /// Every key maps; tuke holds back none of them.
    ///
    /// [`tuinix::KeyCode::BackTab`] has no guest counterpart: the guest spells
    /// it as [`GuestKeyCode::Tab`] with Shift held, so that is what it becomes.
    pub fn to_guest_key(self) -> Option<KeyEvent> {
        let Self::Key { code, ctrl, alt } = self else {
            return None;
        };
        let mut modifiers = Modifiers::new();
        if ctrl {
            modifiers = modifiers.ctrl();
        }
        if alt {
            modifiers = modifiers.alt();
        }
        let code = match code {
            tuinix::KeyCode::Char(c) => GuestKeyCode::Char(c),
            tuinix::KeyCode::Enter => GuestKeyCode::Enter,
            tuinix::KeyCode::Escape => GuestKeyCode::Escape,
            tuinix::KeyCode::Backspace => GuestKeyCode::Backspace,
            tuinix::KeyCode::Tab => GuestKeyCode::Tab,
            tuinix::KeyCode::BackTab => {
                modifiers = modifiers.shift();
                GuestKeyCode::Tab
            }
            tuinix::KeyCode::Delete => GuestKeyCode::Delete,
            tuinix::KeyCode::Insert => GuestKeyCode::Insert,
            tuinix::KeyCode::Up => GuestKeyCode::Up,
            tuinix::KeyCode::Down => GuestKeyCode::Down,
            tuinix::KeyCode::Left => GuestKeyCode::Left,
            tuinix::KeyCode::Right => GuestKeyCode::Right,
            tuinix::KeyCode::Home => GuestKeyCode::Home,
            tuinix::KeyCode::End => GuestKeyCode::End,
            tuinix::KeyCode::PageUp => GuestKeyCode::PageUp,
            tuinix::KeyCode::PageDown => GuestKeyCode::PageDown,
            tuinix::KeyCode::F(n) => GuestKeyCode::Function(n),
        };
        Some(KeyEvent { code, modifiers })
    }
}
