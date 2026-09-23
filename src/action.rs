//! Side effects the Sans I/O core asks the edge to perform.
//!
//! An [`Action`] is an intent, not an effect: [`State::update`](crate::state::State::update)
//! returns these without performing any I/O, and the edge (the `app` module in
//! the binary) carries them out against the real PTY and terminal.

/// A side effect requested by the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a key to the child's PTY.
    SendKey(termnix::KeyEvent),

    /// Send bracketed-paste text to the child's PTY.
    ///
    /// The payload is the pasted text alone, without markers: the edge wraps
    /// it with the session's bracketed-paste mode at enqueue time, so the core
    /// never writes bytes whose form depends on the child's mode.
    SendPaste(String),

    /// Send a mouse event to the child's PTY.
    ///
    /// The event names a button and a grid position, not report bytes: the
    /// edge builds the report with the session's current mouse-reporting mode
    /// at enqueue time, so the core never writes bytes whose form depends on
    /// the child's modes.
    SendMouse(termnix::MouseEvent),

    /// Send raw bytes to the child's PTY (for example a payload the child
    /// asked for as bytes).
    SendBytes(Vec<u8>),

    /// Type a configured string into the child's PTY.
    ///
    /// It is the text of a shortcut key (a [`KeyAction::Shortcut`]), typed the
    /// way pressing its keys on the host keyboard would type it, and it
    /// carries no Enter: the user decides what happens once the text is there.
    ///
    /// The payload is the text alone, not the key events it spells, so the
    /// core writes no `termnix` key code and the child's modes decide how each
    /// character reaches it. The edge turns each character into a key press.
    ///
    /// [`KeyAction::Shortcut`]: crate::layout::KeyAction::Shortcut
    SendShortcut(String),

    /// Resize the child's PTY to the grid area's size.
    ResizeSession(tuinix::Size),

    /// Repaint the screen.
    Redraw,
}
