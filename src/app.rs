use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use orfail::OrFail;

use crate::layout::{KeyCode, KeyPressState, KeyState, Layout};

#[derive(Debug)]
pub struct AppOptions {
    pub cursor_refresh_interval: Duration,
    pub auto_resize: bool,
    pub log_file_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    options: AppOptions,
    keys: Vec<KeyState>,
    pane_index: usize,
    exit: bool,
    offset: tuinix::TerminalPosition,
    log_file: Option<std::fs::File>,
}

impl App {
    pub fn new(layout: Layout, options: AppOptions) -> orfail::Result<Self> {
        let log_file = options
            .log_file_path
            .as_ref()
            .map(|path| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
            })
            .transpose()
            .or_fail()?;

        let mut terminal = tuinix::Terminal::new().or_fail()?;
        terminal.enable_mouse_input().or_fail()?;

        let keys = layout
            .keys
            .iter()
            .map(|k| KeyState::new(k.clone()))
            .collect();

        let mut app = Self {
            terminal,
            options,
            keys,
            pane_index: 0,
            exit: false,
            offset: tuinix::TerminalPosition::default(),
            log_file,
        };

        app.calculate_offset();

        Ok(app)
    }

    pub fn run(mut self) -> orfail::Result<()> {
        self.render().or_fail()?;

        let mut is_last_timeout = false;
        while !self.exit {
            let timeout = self.options.cursor_refresh_interval;
            match self
                .terminal
                .poll_event(&[], &[], Some(timeout))
                .or_fail()?
            {
                Some(tuinix::TerminalEvent::Input(input)) => {
                    self.handle_input(input).or_fail()?;
                    self.render().or_fail()?;
                    is_last_timeout = false;
                }
                Some(tuinix::TerminalEvent::Resize(_)) => {
                    self.calculate_offset();
                    self.render().or_fail()?;
                    is_last_timeout = false;
                }
                None if !is_last_timeout => {
                    // Timeout
                    self.tmux_command("select-pane", &["-t", &format!(".{}", self.pane_index)])
                        .or_fail()?;
                    is_last_timeout = true;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_input(&mut self, input: tuinix::TerminalInput) -> orfail::Result<()> {
        match input {
            tuinix::TerminalInput::Key(key_input) => {
                self.exit = match key_input.code {
                    tuinix::KeyCode::Char('q') => true,
                    tuinix::KeyCode::Char('c') if key_input.ctrl => true,
                    _ => false,
                };
            }
            tuinix::TerminalInput::Mouse(mouse_input) => {
                self.handle_mouse_input(mouse_input).or_fail()?;
            }
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, mouse_input: tuinix::MouseInput) -> orfail::Result<()> {
        if mouse_input.event != tuinix::MouseEvent::LeftRelease {
            return Ok(());
        }

        let adjusted_position = tuinix::TerminalPosition::row_col(
            mouse_input.position.row.saturating_sub(self.offset.row),
            mouse_input.position.col.saturating_sub(self.offset.col),
        );

        let Some(pressed_index) = self
            .keys
            .iter()
            .position(|ks| ks.key.region.contains(adjusted_position))
        else {
            return Ok(());
        };

        if self.keys[pressed_index].key.code.is_modifier() {
            self.handle_modifier_key_pressed(pressed_index).or_fail()?;
        } else {
            self.handle_normal_key_pressed(pressed_index).or_fail()?;
        }

        self.log_key_press(self.keys[pressed_index].key.code)
            .or_fail()?;

        Ok(())
    }

    fn log_key_press(&mut self, key_code: KeyCode) -> orfail::Result<()> {
        if let Some(ref mut file) = self.log_file {
            let timestamp = std::time::UNIX_EPOCH.elapsed().or_fail()?.as_secs_f64();
            writeln!(
                file,
                "{}",
                nojson::object(|f| {
                    f.member("timestamp", timestamp)?;
                    f.member("key", key_code.to_string())
                })
            )
            .or_fail()?;
        }
        Ok(())
    }

    fn reset_pressed_keys(&mut self) {
        for key in &mut self.keys {
            if key.press == KeyPressState::Pressed {
                key.press = KeyPressState::Neutral;
            }
        }
    }

    fn tmux_command(&self, command: &str, args: &[&str]) -> orfail::Result<()> {
        Command::new("tmux")
            .arg(command)
            .args(args)
            .output()
            .or_fail()?;
        Ok(())
    }

    fn handle_modifier_key_pressed(&mut self, i: usize) -> orfail::Result<()> {
        self.reset_pressed_keys();

        match self.keys[i].press {
            KeyPressState::Neutral => {
                self.keys[i].press = KeyPressState::OneshotActivated;
            }
            KeyPressState::Pressed => {
                self.keys[i].press = KeyPressState::OneshotActivated;
            }
            KeyPressState::Activated => {
                self.keys[i].press = KeyPressState::Neutral;
            }
            KeyPressState::OneshotActivated => {
                self.keys[i].press = KeyPressState::Activated;
            }
        }

        Ok(())
    }

    fn handle_normal_key_pressed(&mut self, i: usize) -> orfail::Result<()> {
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
        self.keys[i].press = KeyPressState::Pressed;

        let mut code = self.keys[i].key.code;
        let mut key_string = String::new();
        if code.is_modifiable() {
            if self.is_ctrl_pressed() {
                key_string.push_str("C-");
            }
            if self.is_alt_pressed() {
                key_string.push_str("M-");
            }
        }
        if self.is_shift_pressed() {
            code = self.keys[i].key.shift_code;
        }

        // Escape semicolons for tmux
        let code_str = code.to_string();
        let escaped_code = code_str.replace(';', "\\;");
        key_string.push_str(&escaped_code);

        self.tmux_command(
            "send-keys",
            &["-t", &format!(".{}", self.pane_index), &key_string],
        )
        .or_fail()?;

        Ok(())
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

    fn is_shift_active(&self) -> bool {
        self.keys.iter().any(|k| {
            k.key.code == KeyCode::Shift
                && matches!(
                    k.press,
                    KeyPressState::OneshotActivated | KeyPressState::Activated
                )
        })
    }

    fn calculate_offset(&mut self) {
        let terminal_size = self.terminal.size();
        let mut actual_frame_size = tuinix::TerminalSize::default();

        for key_state in &self.keys {
            actual_frame_size.rows = actual_frame_size
                .rows
                .max(key_state.key.region.position.row + key_state.key.region.size.rows);
            actual_frame_size.cols = actual_frame_size
                .cols
                .max(key_state.key.region.position.col + key_state.key.region.size.cols);
        }

        // Calculate centering offset
        let offset_row = (terminal_size.rows.saturating_sub(actual_frame_size.rows)) / 2;
        let offset_col = (terminal_size.cols.saturating_sub(actual_frame_size.cols)) / 2;

        self.offset = tuinix::TerminalPosition::row_col(offset_row, offset_col);
    }

    fn render(&mut self) -> orfail::Result<()> {
        let terminal_size = self.terminal.size();

        if self.options.auto_resize {
            let required_rows = self
                .keys
                .iter()
                .map(|k| k.key.region.bottom_left().row + 1)
                .max()
                .unwrap_or_default();
            if terminal_size.rows != required_rows {
                self.tmux_command("resize-pane", &["-y", &required_rows.to_string()])
                    .or_fail()?;
            }
        }

        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(terminal_size);
        let shift = self.is_shift_active();

        for key_state in &mut self.keys {
            let key_frame = key_state.to_frame(shift).or_fail()?;
            frame.draw(key_state.key.region.position, &key_frame);
        }

        let mut centered_frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(terminal_size);
        centered_frame.draw(self.offset, &frame);

        self.terminal.draw(centered_frame).or_fail()?;

        Ok(())
    }
}
