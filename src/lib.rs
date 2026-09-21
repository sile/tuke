//! `tuke`'s Sans I/O core.
//!
//! This crate holds everything that does not touch a file descriptor: the
//! layout model ([`layout`]), the terminal/keyboard geometry ([`geometry`]),
//! the pure transition function ([`state::State::update`]) with its input
//! [`event::Event`]s and requested [`action::Action`]s, and the pure renderer
//! ([`render::frame`]). The binary drives the real PTY and terminal.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod action;
pub mod error;
pub mod event;
pub mod geometry;
mod jsonc;
pub mod layout;
pub mod render;
pub mod state;

pub use error::{Error, Result};
