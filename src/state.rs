//! The Sans I/O core: soft-keyboard state and its transition function.
//!
//! [`State`] owns the loaded layout, the per-key press state, and the send
//! preview. [`State::update`] is a pure transition (`Event` in, `Vec<Action>`
//! out) that never performs I/O; the edge carries the returned actions out
//! against the real PTY and terminal.

use crate::action::Action;
use crate::event::Event;
use crate::geometry;
use crate::layout::{KeyCode, KeyPressState, KeyState, Layout, Preview};

/// The soft keyboard's state and its pure transition function.
#[derive(Debug)]
pub struct State {
    keys: Vec<KeyState>,
    preview: Option<Preview>,
    terminal_size: tuinix::Size,
    /// Where the layout's origin sits in screen coordinates.
    offset: tuinix::Position,
    /// The grid area the child's PTY should be sized to.
    grid_size: tuinix::Size,
    exit: bool,
}

impl State {
    /// Builds the initial state from a layout and the current terminal size.
    pub fn new(layout: Layout, terminal_size: tuinix::Size) -> Self {
        let keys = layout
            .keys
            .iter()
            .map(|key| KeyState::new(key.clone()))
            .collect();
        let mut state = Self {
            keys,
            preview: layout.preview,
            terminal_size,
            offset: tuinix::Position::ORIGIN,
            grid_size: tuinix::Size::default(),
            exit: false,
        };
        state.recompute_geometry();
        state
    }

    /// Whether the core has asked the application to quit.
    pub fn should_exit(&self) -> bool {
        self.exit
    }

    /// The keyboard's keys and their press states.
    pub fn keys(&self) -> &[KeyState] {
        &self.keys
    }

    /// The send preview, if the layout defines one.
    pub fn preview(&self) -> Option<&Preview> {
        self.preview.as_ref()
    }

    /// The screen position of the layout's origin (top-left corner).
    pub fn offset(&self) -> tuinix::Position {
        self.offset
    }

    /// The size of the terminal grid area the child's PTY is sized to.
    pub fn grid_size(&self) -> tuinix::Size {
        self.grid_size
    }

    /// Whether Shift is currently active (one-shot or held).
    pub fn is_shift_active(&self) -> bool {
        self.keys.iter().any(|k| {
            k.key.code == KeyCode::Shift
                && matches!(
                    k.press,
                    KeyPressState::OneshotActivated | KeyPressState::Activated
                )
        })
    }

    /// The largest bottom edge (`row + rows`) over the keys and the preview.
    fn layout_rows(&self) -> usize {
        self.keys
            .iter()
            .map(|k| k.key.region)
            .chain(self.preview.iter().map(|p| p.region))
            .map(|r| r.position.row + r.size.rows)
            .max()
            .unwrap_or_default()
    }

    /// The largest right edge (`col + cols`) over the keys and the preview.
    fn layout_cols(&self) -> usize {
        self.keys
            .iter()
            .map(|k| k.key.region)
            .chain(self.preview.iter().map(|p| p.region))
            .map(|r| r.position.col + r.size.cols)
            .max()
            .unwrap_or_default()
    }

    /// Recomputes the keyboard offset and the grid size from the terminal size
    /// and the layout's extent.
    fn recompute_geometry(&mut self) {
        let keyboard_rows = geometry::keyboard_rows(self.layout_rows());
        let grid_rows = geometry::grid_rows(self.terminal_size.rows, keyboard_rows);
        let offset_col = geometry::keyboard_offset_col(self.terminal_size.cols, self.layout_cols());

        // The keyboard is bottom-aligned, so its origin is the row just below
        // the grid. `grid_rows` is already terminal height minus keyboard
        // height (saturating), which is exactly that row.
        self.offset = tuinix::Position {
            row: grid_rows,
            col: offset_col,
        };
        self.grid_size = tuinix::Size {
            rows: grid_rows,
            cols: self.terminal_size.cols,
        };
    }

    /// Applies an event, returning the actions it asks for.
    pub fn update(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Resize { size } => self.on_resize(size),
            Event::PointerRelease { position } => self.on_pointer_release(position),
            Event::Key { code, ctrl } => self.on_key(code, ctrl),
        }
    }

    fn on_resize(&mut self, size: tuinix::Size) -> Vec<Action> {
        if size == self.terminal_size {
            return Vec::new();
        }
        self.terminal_size = size;
        self.recompute_geometry();
        vec![Action::ResizeSession(self.grid_size), Action::Redraw]
    }

    fn on_key(&mut self, code: tuinix::KeyCode, ctrl: bool) -> Vec<Action> {
        let quit = matches!(code, tuinix::KeyCode::Char('q'))
            || (ctrl && matches!(code, tuinix::KeyCode::Char('c')));
        if quit {
            self.exit = true;
            return vec![Action::Quit];
        }
        Vec::new()
    }

    fn on_pointer_release(&mut self, position: tuinix::Position) -> Vec<Action> {
        // A release above or left of the keyboard is outside it entirely.
        // Saturating subtraction would fold such a point onto row or column 0
        // and hit a key that is not under the pointer.
        let (Some(row), Some(col)) = (
            position.row.checked_sub(self.offset.row),
            position.col.checked_sub(self.offset.col),
        ) else {
            return Vec::new();
        };
        let local = tuinix::Position { row, col };

        let Some(index) = self
            .keys
            .iter()
            .position(|ks| ks.key.region.contains(local))
        else {
            return Vec::new();
        };

        if self.keys[index].key.code.is_modifier() {
            self.press_modifier(index);
            vec![Action::Redraw]
        } else {
            self.press_normal(index)
        }
    }

    /// Clears the transient `Pressed` highlight from every key.
    fn reset_pressed_keys(&mut self) {
        for key in &mut self.keys {
            if key.press == KeyPressState::Pressed {
                key.press = KeyPressState::Neutral;
            }
        }
    }

    fn press_modifier(&mut self, index: usize) {
        self.reset_pressed_keys();
        match self.keys[index].press {
            KeyPressState::Neutral => {
                self.keys[index].press = KeyPressState::OneshotActivated;
            }
            KeyPressState::Pressed => {
                self.keys[index].press = KeyPressState::OneshotActivated;
            }
            KeyPressState::Activated => {
                self.keys[index].press = KeyPressState::Neutral;
            }
            KeyPressState::OneshotActivated => {
                self.keys[index].press = KeyPressState::Activated;
            }
        }
    }

    /// Updates the modifier-held/one-shot state for a normal key press and
    /// returns the key to send.
    fn press_normal(&mut self, index: usize) -> Vec<Action> {
        for key in &mut self.keys {
            match key.press {
                KeyPressState::Neutral => {}
                KeyPressState::Pressed => {
                    key.press = KeyPressState::Neutral;
                }
                KeyPressState::Activated => {}
                KeyPressState::OneshotActivated => {
                    key.press = KeyPressState::Pressed;
                }
            }
        }
        self.keys[index].press = KeyPressState::Pressed;

        let shift = self.is_shift_pressed();
        let mut code = self.keys[index].key.code;
        if shift {
            code = self.keys[index].key.shift_code;
        }

        // Modifiers only combine with keys that accept them; a modifier key
        // itself is never sent on its own.
        let modifiable = code.is_modifiable();
        let ctrl = modifiable && self.is_ctrl_pressed();
        let alt = modifiable && self.is_alt_pressed();

        // `None` only happens for modifier codes, which never reach here (they
        // go through `press_modifier`), so a normal key always maps.
        let Some(term_code) = code.to_termnix() else {
            return vec![Action::Redraw];
        };

        let mut modifiers = termnix::Modifiers::new();
        if ctrl {
            modifiers = modifiers.ctrl();
        }
        if alt {
            modifiers = modifiers.alt();
        }
        // BackTab is the shifted form of Tab, so the child needs Shift set.
        if code == KeyCode::BackTab {
            modifiers = modifiers.shift();
        }

        if let Some(preview) = &mut self.preview {
            preview.on_key_sent(code, ctrl, alt);
        }

        vec![
            Action::SendKey(termnix::KeyEvent {
                code: term_code,
                modifiers,
            }),
            Action::Redraw,
        ]
    }

    fn is_ctrl_pressed(&self) -> bool {
        self.keys.iter().any(|k| {
            k.key.code == KeyCode::Ctrl
                && matches!(k.press, KeyPressState::Pressed | KeyPressState::Activated)
        })
    }

    fn is_alt_pressed(&self) -> bool {
        self.keys.iter().any(|k| {
            k.key.code == KeyCode::Alt
                && matches!(k.press, KeyPressState::Pressed | KeyPressState::Activated)
        })
    }

    fn is_shift_pressed(&self) -> bool {
        self.keys.iter().any(|k| {
            k.key.code == KeyCode::Shift
                && matches!(k.press, KeyPressState::Pressed | KeyPressState::Activated)
        })
    }
}
