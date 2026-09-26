//! The Sans I/O core: soft-keyboard state and its transition function.
//!
//! [`State`] owns the loaded layout and the per-key press state.
//! [`State::update`] is a pure transition (`Event` in, `Vec<Action>` out) that
//! never performs I/O; the edge carries the returned actions out against the
//! real PTY and terminal.

use crate::action::Action;
use crate::event::Event;
use crate::layout::{KeyAction, KeyCode, KeyPressState, KeyState, KeyboardPos, LayoutSet};

/// The soft keyboard's state and its pure transition function.
#[derive(Debug)]
pub struct State {
    /// Every layout in the set, so a `switch_to` key can look up its target.
    layouts: LayoutSet,
    /// The name of the layout currently shown.
    current: String,
    keys: Vec<KeyState>,
    terminal_size: tuinix::Size,
    /// Where the layout's origin sits in screen coordinates.
    offset: tuinix::Position,
    /// The grid area the child's PTY should be sized to.
    grid_size: tuinix::Size,
    /// Where the keyboard floats, from the current layout's `keyboard_pos`.
    ///
    /// It is kept as the layout file gave it (terminal bottom-left origin, not
    /// resolved against a size), so a resize can resolve it again and the
    /// keyboard keeps its offset from the corner it was pinned to.
    keyboard_pos: KeyboardPos,
    /// The mouse button currently held, as the guest was last told.
    ///
    /// The guest's protocol spells a drag as "moved while this button is
    /// held", but the host reports a drag without naming the button, so the
    /// core remembers which one went down and hands it back at the drag. It is
    /// cleared on release, and a drag that arrives with nothing held is sent
    /// as a bare move rather than guessed at.
    held_button: Option<tuinix::MouseInputKind>,
}

impl State {
    /// Builds the initial state from a layout set and the current terminal
    /// size.
    ///
    /// The keyboard floats: its position comes from the first layout's
    /// `keyboard_pos`, the grid keeps the whole terminal, and the keyboard is
    /// painted over it.
    pub fn new(layouts: LayoutSet, terminal_size: tuinix::Size) -> Self {
        let current = layouts.first_name().to_string();
        let mut state = Self {
            layouts,
            current,
            keys: Vec::new(),
            terminal_size,
            offset: tuinix::Position::ORIGIN,
            grid_size: tuinix::Size::default(),
            keyboard_pos: KeyboardPos::ORIGIN,
            held_button: None,
        };
        state.show_current_layout();
        state.recompute_geometry();
        state
    }

    /// Rebuilds the keys from the current layout.
    ///
    /// Switching layouts replaces the whole keyboard, so this is what both
    /// startup and a `switch_to` press go through: the new layout's keys start
    /// neutral.
    fn show_current_layout(&mut self) {
        let layout = self
            .layouts
            .get(&self.current)
            .expect("the current layout name always names a layout in the set");
        self.keys = layout
            .keys
            .iter()
            .map(|key| KeyState::new(key.clone()))
            .collect();
        self.keyboard_pos = layout.keyboard_pos;
    }

    /// The name of the layout currently shown.
    pub fn current_layout_name(&self) -> &str {
        &self.current
    }

    /// The keyboard's keys and their press states.
    pub fn keys(&self) -> &[KeyState] {
        &self.keys
    }

    /// The screen position of the layout's origin (top-left corner).
    pub fn offset(&self) -> tuinix::Position {
        self.offset
    }

    /// The size of the terminal grid area the child's PTY is sized to.
    pub fn grid_size(&self) -> tuinix::Size {
        self.grid_size
    }

    /// The keyboard's bounding box: the extent of every key.
    pub fn layout_size(&self) -> tuinix::Size {
        tuinix::Size {
            rows: self.layout_rows(),
            cols: self.layout_cols(),
        }
    }

    /// Whether Shift is currently active (one-shot or held).
    pub fn is_shift_active(&self) -> bool {
        self.keys.iter().any(|k| {
            matches!(
                k.key.action,
                KeyAction::Send { code, .. } if code == KeyCode::Shift
            ) && matches!(
                k.press,
                KeyPressState::OneshotActivated | KeyPressState::Activated
            )
        })
    }

    /// The largest bottom edge (`row + rows`) over the keys.
    fn layout_rows(&self) -> usize {
        self.keys
            .iter()
            .map(|k| k.key.region)
            .map(|r| r.position.row + r.size.rows)
            .max()
            .unwrap_or_default()
    }

    /// The largest right edge (`col + cols`) over the keys.
    fn layout_cols(&self) -> usize {
        self.keys
            .iter()
            .map(|k| k.key.region)
            .map(|r| r.position.col + r.size.cols)
            .max()
            .unwrap_or_default()
    }

    /// Recomputes the keyboard offset and the grid size from the terminal size
    /// and the layout's extent.
    ///
    /// The keyboard floats: the grid keeps the whole terminal, and the
    /// keyboard is anchored at the corner `keyboard_pos` names. The anchor is
    /// resolved against the current terminal size, so a resize moves the
    /// keyboard with the corner it was pinned to.
    fn recompute_geometry(&mut self) {
        let layout_rows = self.layout_rows();
        let anchor = self.keyboard_pos.to_screen(self.terminal_size);
        // `anchor.row` is the keyboard's last row, so the origin is
        // `layout_rows - 1` rows above it. Subtracting the whole `layout_rows`
        // would put the origin one row too high and the keyboard one row above
        // the anchor.
        self.offset = tuinix::Position {
            row: anchor.row.saturating_add(1).saturating_sub(layout_rows),
            col: anchor.col,
        };
        self.grid_size = self.terminal_size;
    }

    /// Applies an event, returning the actions it asks for.
    pub fn update(&mut self, event: Event) -> Vec<Action> {
        match event {
            Event::Resize { size } => self.on_resize(size),
            Event::PointerRelease { position } => self.on_pointer_release(position),
            Event::Mouse { .. } => self.on_mouse(event),
            Event::Key { .. } => self.on_key(event),
            Event::Paste { .. } => self.on_paste(event),
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

    fn on_mouse(&mut self, event: Event) -> Vec<Action> {
        let Some(position) = event.mouse_position() else {
            return Vec::new();
        };
        let inside_key = self.within_keyboard(position);

        // A left-button release on a key is the one mouse gesture the keyboard
        // claims: it presses the key under the pointer.
        if inside_key && event.mouse_kind() == Some(tuinix::MouseInputKind::LeftRelease) {
            return self.on_pointer_release(position);
        }

        // The wheel always belongs to the child, wherever the pointer is: a
        // turn over the keyboard still scrolls the child's scrollback, because
        // the soft keys have nothing to scroll and swallowing the turn would
        // make the wheel dead over half the screen.
        let wheel = matches!(
            event.mouse_kind(),
            Some(tuinix::MouseInputKind::ScrollUp | tuinix::MouseInputKind::ScrollDown)
        );

        // Everything else that lands on the keyboard is the keyboard's, not
        // the child's, and is swallowed: a press or release on the board must
        // not reach the child as a click, or a tap on a soft key would arrive
        // as a click on whatever the child has under that cell as well as as a
        // key. The whole bounding box is the keyboard, not just the keys: the
        // gaps between keys are the board's own backing, so a click that lands
        // there is swallowed too rather than showing through to the child.
        // A drag whose button went down outside the keyboard is the exception:
        // it belongs to wherever the gesture started, so it keeps being the
        // child's even once it crosses onto the board.
        let childs_drag =
            event.mouse_kind() == Some(tuinix::MouseInputKind::Drag) && self.held_button.is_some();
        if inside_key && !wheel && !childs_drag {
            return Vec::new();
        }

        if let Some(kind) = event.mouse_kind() {
            match kind {
                tuinix::MouseInputKind::LeftPress
                | tuinix::MouseInputKind::MiddlePress
                | tuinix::MouseInputKind::RightPress => self.held_button = Some(kind),
                tuinix::MouseInputKind::LeftRelease
                | tuinix::MouseInputKind::MiddleRelease
                | tuinix::MouseInputKind::RightRelease => self.held_button = None,
                tuinix::MouseInputKind::ScrollUp
                | tuinix::MouseInputKind::ScrollDown
                | tuinix::MouseInputKind::Drag => {}
            }
        }
        event
            .to_guest_mouse(self.held_button)
            .map_or_else(Vec::new, |mouse| vec![Action::SendMouse(mouse)])
    }

    /// Whether a screen position lands inside the keyboard's bounding box.
    ///
    /// The keyboard is the whole box it paints: its backing and border, not
    /// just the keys. A click in the gaps between keys is on the board, not on
    /// the grid showing through, so this is the rectangle the mouse is routed
    /// by — a hit anywhere in the box is the keyboard's.
    fn within_keyboard(&self, position: tuinix::Position) -> bool {
        let Some(local) = self.screen_to_layout(position) else {
            return false;
        };
        tuinix::Region {
            position: tuinix::Position::ORIGIN,
            size: self.layout_size(),
        }
        .contains(local)
    }

    /// Translates a screen position into the layout's coordinate system.
    ///
    /// Returns `None` when the position is above or left of the keyboard's
    /// origin, where the layout has no coordinates at all.
    fn screen_to_layout(&self, position: tuinix::Position) -> Option<tuinix::Position> {
        Some(tuinix::Position {
            row: position.row.checked_sub(self.offset.row)?,
            col: position.col.checked_sub(self.offset.col)?,
        })
    }

    fn on_key(&mut self, event: Event) -> Vec<Action> {
        // Every host key belongs to the child: tuke reserves none of its own,
        // so it has no quit key and nothing to filter. `q` reaches the child
        // as `q`, and `C-c` as an interrupt, exactly as they would without
        // tuke in the way.
        event
            .to_guest_key()
            .map_or_else(Vec::new, |key| vec![Action::SendKey(key)])
    }

    fn on_paste(&mut self, event: Event) -> Vec<Action> {
        // A paste is text the child asked for, exactly like a key is. It is
        // sent as one input rather than as the key presses it spells, so the
        // child can tell a paste from typing and its bracketed-paste mode is
        // honoured on the way out.
        //
        // The keyboard's own state is left alone: the paste did not go through
        // any soft key, so no highlight is armed or consumed by it.
        event
            .to_guest_paste()
            .map_or_else(Vec::new, |text| vec![Action::SendPaste(text)])
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

        if matches!(self.keys[index].key.action, KeyAction::Send { code, .. } if code.is_modifier())
        {
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
    ///
    /// A key that switches layouts is handled here too: a press on it replaces
    /// the keyboard with its target layout and sends nothing, so it has no
    /// press state to hold and no code to send. The switch happens on the
    /// press itself, not on a later key, so the new layout is what the user
    /// sees as soon as the key is released.
    ///
    /// A shortcut key is handled here as well: its text does not depend on the
    /// modifier keys the way a single code does (`C-a` is not `Ctrl` applied
    /// to a string), so it is handed to the edge whole rather than being run
    /// through the shift/Ctrl/Alt state below, and it holds no press state.
    fn press_normal(&mut self, index: usize) -> Vec<Action> {
        let (code, shift_code) = match &self.keys[index].key.action {
            KeyAction::Send { code, shift_code } => (*code, *shift_code),
            KeyAction::Shortcut { text, .. } => {
                // The text is taken before the press state is cleared: the
                // `match` still borrows the key it came from, so mutating the
                // keys here would need it while the borrow is live.
                let text = text.clone();
                self.reset_pressed_keys();
                return vec![Action::SendShortcut(text), Action::Redraw];
            }
            KeyAction::Switch { to } => {
                // A set read from a file has already checked every switch, so
                // this only misses for a set built by `LayoutSet::from_named`,
                // which does not. The keyboard then stays as it is rather than
                // the press doing nothing at all.
                if self.layouts.get(to).is_some() {
                    self.current = to.clone();
                    self.show_current_layout();
                    // The new layout can be a different size, and the keyboard
                    // is anchored by its bottom edge: it keeps the bottom of
                    // the screen and grows upward. Recomputing here resolves
                    // the offset and the grid against the new extent, so a
                    // taller board reaches up rather than the bottom edge
                    // drifting off the screen.
                    let previous_grid = self.grid_size;
                    self.recompute_geometry();
                    let mut actions = vec![Action::Redraw];
                    if self.grid_size != previous_grid {
                        actions.push(Action::ResizeSession(self.grid_size));
                    }
                    return actions;
                }
                return Vec::new();
            }
        };

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
        let code = if shift { shift_code } else { code };

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

        vec![
            Action::SendKey(termnix::KeyEvent {
                code: term_code,
                modifiers,
            }),
            Action::Redraw,
        ]
    }

    fn is_ctrl_pressed(&self) -> bool {
        self.is_modifier_pressed(KeyCode::Ctrl)
    }

    fn is_alt_pressed(&self) -> bool {
        self.is_modifier_pressed(KeyCode::Alt)
    }

    fn is_shift_pressed(&self) -> bool {
        self.is_modifier_pressed(KeyCode::Shift)
    }

    /// Whether the soft key that carries the `modifier` code is held down.
    fn is_modifier_pressed(&self, modifier: KeyCode) -> bool {
        self.keys.iter().any(|k| {
            matches!(
                k.key.action,
                KeyAction::Send { code, .. } if code == modifier
            ) && matches!(k.press, KeyPressState::Pressed | KeyPressState::Activated)
        })
    }
}
