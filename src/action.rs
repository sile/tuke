//! Side effects the Sans I/O core asks the edge to perform.
//!
//! An [`Action`] is an intent, not an effect: [`State::update`](crate::state::State::update)
//! returns these without performing any I/O, and the edge (the `app` module in
//! the binary) carries them out against the real PTY and terminal.

use termnix::KeyEvent;

/// A side effect requested by the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Send a key to the child's PTY.
    SendKey(KeyEvent),

    /// Send raw bytes to the child's PTY (for example a paste payload).
    SendBytes(Vec<u8>),

    /// Resize the child's PTY to the grid area's size.
    ResizeSession(tuinix::Size),

    /// Repaint the screen.
    Redraw,

    /// Exit the application.
    Quit,
}
