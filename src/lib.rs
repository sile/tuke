//! `tuke`'s Sans I/O core.
//!
//! This crate holds everything that does not touch a file descriptor: the
//! layout model ([`Layout`]), the terminal/keyboard geometry, the pure
//! transition function ([`State::update`]) with its input [`Event`]s and
//! requested [`Action`]s, and the pure renderer ([`screen_frame`]). The binary
//! drives the real PTY and terminal.
//!
//! The types and functions callers work with are re-exported here, so they are
//! named at the crate root ([`State`], [`Layout`], [`KeyCode`],
//! [`screen_frame`], …); the modules themselves stay private.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod action;
mod error;
mod event;
mod geometry;
mod jsonc;
mod layout;
mod render;
mod state;

pub use action::Action;
pub use error::{Error, Result};
pub use event::Event;
pub use geometry::{from_termnix_size, to_termnix_size};
pub use layout::{
    Key, KeyAction, KeyCode, KeyPressState, KeyState, KeyboardPos, Layout, LayoutSet, NamedLayout,
    Preview,
};
pub use render::{CURSOR_CLEARANCE, screen_cursor, screen_frame};
pub use state::State;
