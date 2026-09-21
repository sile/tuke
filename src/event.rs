//! Inputs the edge feeds into the Sans I/O core.

/// An event delivered to [`State::update`](crate::state::State::update).
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// A bracketed paste arrived from the host terminal.
    ///
    /// The host wrapped the bytes between `CSI 200~` and `CSI 201~`, so they
    /// are the text of a paste rather than a burst of typing. They are carried
    /// as bytes rather than as a string because the input decoder does not
    /// promise the payload is UTF-8, and the child is the only reader that can
    /// decide what to make of it.
    Paste {
        /// The pasted text, exactly as it arrived.
        bytes: Vec<u8>,
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
    /// it as [`termnix::KeyCode::Tab`] with Shift held, so that is what it
    /// becomes.
    pub fn to_guest_key(self) -> Option<termnix::KeyEvent> {
        use termnix::KeyCode as GuestKeyCode;

        let Self::Key { code, ctrl, alt } = self else {
            return None;
        };
        let mut modifiers = termnix::Modifiers::new();
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
        Some(termnix::KeyEvent { code, modifiers })
    }

    /// Converts a host paste into the pasted text to forward.
    ///
    /// A paste is one piece of text, so the core hands it on whole rather than
    /// as the key presses it spells: the child is told the bytes came from a
    /// paste, and the edge re-marks them with the child's own bracketed-paste
    /// mode. The host's markers are not forwarded — they belong to the host's
    /// protocol with tuke, and the guest's belong to the guest's protocol with
    /// tuke — so the payload travels on its own and is marked again on the way
    /// out if the guest asked for marks.
    ///
    /// Bytes that are not valid UTF-8 have no text to hand on: the decoder
    /// reports a paste body as bytes because a paste is whatever the terminal
    /// sent, but a guest paste is a `&str`, so tuke drops a paste it cannot
    /// represent rather than guessing an encoding. The keyboard does not stand
    /// in for it, because fabricating key presses would enter bytes the user
    /// never typed.
    pub fn to_guest_paste(self) -> Option<String> {
        let Self::Paste { bytes } = self else {
            return None;
        };
        String::from_utf8(bytes).ok()
    }
}
