//! Inputs the edge feeds into the Sans I/O core.

/// An event delivered to [`State::update`](crate::State::update).
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

    /// A mouse event happened at `position`, in screen coordinates
    /// (zero-based, measured from the terminal's top-left corner).
    ///
    /// tuke forwards nearly all of these to the child, so the child can use
    /// its own mouse reporting: a click, a drag, and a wheel turn inside the
    /// grid area belong to the child, not to the soft keyboard. Only what
    /// lands on the keyboard belongs to tuke, and only a left-button release
    /// does anything there (it presses the key under the pointer).
    ///
    /// The event is carried whole rather than pre-decided, because whether it
    /// is the keyboard's or the child's depends on the layout's extent, which
    /// is state the core holds; the edge only translates the toolkit types.
    Mouse {
        /// What the mouse did.
        kind: tuinix::MouseInputKind,
        /// Where it happened.
        position: tuinix::Position,
        /// Whether Ctrl was held.
        ctrl: bool,
        /// Whether Alt was held.
        alt: bool,
        /// Whether Shift was held.
        shift: bool,
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
    /// Returns what a mouse event did, or `None` for any other event.
    ///
    /// The core reads the event before it converts it, because whether the
    /// event belongs to the child at all depends on where it happened and
    /// only the core knows the layout's extent.
    pub fn mouse_kind(&self) -> Option<tuinix::MouseInputKind> {
        match *self {
            Self::Mouse { kind, .. } => Some(kind),
            _ => None,
        }
    }

    /// Returns where a mouse event happened, or `None` for any other event.
    pub fn mouse_position(&self) -> Option<tuinix::Position> {
        match *self {
            Self::Mouse { position, .. } => Some(position),
            _ => None,
        }
    }

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
            tuinix::KeyCode::Char(c) => termnix::KeyCode::Char(c),
            tuinix::KeyCode::Enter => termnix::KeyCode::Enter,
            tuinix::KeyCode::Escape => termnix::KeyCode::Escape,
            tuinix::KeyCode::Backspace => termnix::KeyCode::Backspace,
            tuinix::KeyCode::Tab => termnix::KeyCode::Tab,
            tuinix::KeyCode::BackTab => {
                modifiers = modifiers.shift();
                termnix::KeyCode::Tab
            }
            tuinix::KeyCode::Delete => termnix::KeyCode::Delete,
            tuinix::KeyCode::Insert => termnix::KeyCode::Insert,
            tuinix::KeyCode::Up => termnix::KeyCode::Up,
            tuinix::KeyCode::Down => termnix::KeyCode::Down,
            tuinix::KeyCode::Left => termnix::KeyCode::Left,
            tuinix::KeyCode::Right => termnix::KeyCode::Right,
            tuinix::KeyCode::Home => termnix::KeyCode::Home,
            tuinix::KeyCode::End => termnix::KeyCode::End,
            tuinix::KeyCode::PageUp => termnix::KeyCode::PageUp,
            tuinix::KeyCode::PageDown => termnix::KeyCode::PageDown,
            tuinix::KeyCode::F(n) => termnix::KeyCode::Function(n),
        };
        Some(termnix::KeyEvent { code, modifiers })
    }

    /// Converts a host mouse event into the guest mouse event to forward.
    ///
    /// `held` is the button the core believes is down, which the guest has to
    /// be told about: a drag reports the button being held, but the host's
    /// [`tuinix::MouseInputKind::Drag`] does not name it, so the core tracks
    /// it and passes it in. It is `None` when the host reports a drag it
    /// never saw pressed, in which case the move cannot be a drag and is sent
    /// as a bare move.
    ///
    /// The wheel is spelled in the guest as a press of a wheel button: it has
    /// no release, and the guest's own protocol has always carried it as a
    /// press, so mapping it to one keeps the guest from waiting for a release
    /// that will never come.
    pub fn to_guest_mouse(
        self,
        held: Option<tuinix::MouseInputKind>,
    ) -> Option<termnix::MouseEvent> {
        let Self::Mouse {
            kind,
            position,
            ctrl,
            alt,
            shift,
        } = self
        else {
            return None;
        };

        let mut modifiers = termnix::Modifiers::new();
        if ctrl {
            modifiers = modifiers.ctrl();
        }
        if alt {
            modifiers = modifiers.alt();
        }
        if shift {
            modifiers = modifiers.shift();
        }

        let kind = match kind {
            tuinix::MouseInputKind::LeftPress => {
                termnix::MouseEventKind::Press(termnix::MouseButton::Left)
            }
            tuinix::MouseInputKind::LeftRelease => {
                termnix::MouseEventKind::Release(termnix::MouseButton::Left)
            }
            tuinix::MouseInputKind::MiddlePress => {
                termnix::MouseEventKind::Press(termnix::MouseButton::Middle)
            }
            tuinix::MouseInputKind::MiddleRelease => {
                termnix::MouseEventKind::Release(termnix::MouseButton::Middle)
            }
            tuinix::MouseInputKind::RightPress => {
                termnix::MouseEventKind::Press(termnix::MouseButton::Right)
            }
            tuinix::MouseInputKind::RightRelease => {
                termnix::MouseEventKind::Release(termnix::MouseButton::Right)
            }
            tuinix::MouseInputKind::ScrollUp => {
                termnix::MouseEventKind::Press(termnix::MouseButton::WheelUp)
            }
            tuinix::MouseInputKind::ScrollDown => {
                termnix::MouseEventKind::Press(termnix::MouseButton::WheelDown)
            }
            tuinix::MouseInputKind::Drag => termnix::MouseEventKind::Motion {
                button: held.map(guest_button),
            },
        };

        // The grid starts at the screen's top-left corner (`row` 0, `col` 0)
        // and spans the full terminal width, so a screen position is already
        // the guest's grid position. Only the keyboard is indented, and its
        // events never reach here.
        let position = termnix::Position {
            row: position.row as u16,
            col: position.col as u16,
        };

        Some(termnix::MouseEvent {
            kind,
            position,
            modifiers,
        })
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

/// Names the guest's button for a host press or release.
///
/// Only the three buttons that can be held are named: a wheel turn has no
/// held state, and the host reports it as a press that never becomes a drag,
/// so a drag following it is a bare move.
fn guest_button(kind: tuinix::MouseInputKind) -> termnix::MouseButton {
    match kind {
        tuinix::MouseInputKind::LeftPress | tuinix::MouseInputKind::LeftRelease => {
            termnix::MouseButton::Left
        }
        tuinix::MouseInputKind::MiddlePress | tuinix::MouseInputKind::MiddleRelease => {
            termnix::MouseButton::Middle
        }
        tuinix::MouseInputKind::RightPress | tuinix::MouseInputKind::RightRelease => {
            termnix::MouseButton::Right
        }
        tuinix::MouseInputKind::ScrollUp => termnix::MouseButton::WheelUp,
        tuinix::MouseInputKind::ScrollDown => termnix::MouseButton::WheelDown,
        // A drag names no button of its own; the held one is looked up
        // separately and this arm is never reached for a real drag.
        tuinix::MouseInputKind::Drag => termnix::MouseButton::Left,
    }
}
