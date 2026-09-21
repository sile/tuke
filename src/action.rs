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

    /// Send raw bytes to the child's PTY (for example a payload the child
    /// asked for as bytes).
    SendBytes(Vec<u8>),

    /// Resize the child's PTY to the grid area's size.
    ResizeSession(tuinix::Size),

    /// Repaint the screen.
    Redraw,
}
