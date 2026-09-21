//! Inputs the edge feeds into the Sans I/O core.

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

    /// A key that tuke itself interprets was pressed.
    ///
    /// Soft-keyboard keys are delivered as [`Event::PointerRelease`]; this is
    /// for the host keys tuke handles directly (for example to quit).
    Key {
        /// The pressed key.
        code: tuinix::KeyCode,
        /// Whether Ctrl was held.
        ctrl: bool,
    },
}
