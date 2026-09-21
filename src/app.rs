use std::io::{Read, Write};
use std::time::Duration;

use crate::error::Result;
use crate::layout::{KeyCode, KeyPressState, KeyState, Layout, Preview};
use crate::tmux_client::TmuxClient;

/// How long to wait for the rest of an escape sequence before treating a lone
/// `ESC` byte as the Escape key.
const ESCAPE_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Debug)]
pub struct AppOptions {
    pub cursor_refresh_interval: Duration,
    pub auto_resize: bool,
}

#[derive(Debug)]
pub struct App {
    driver: tuinix::TerminalDriver,
    input: tuinix::InputDecoder,
    prev_frame: Option<tuinix::Frame>,
    options: AppOptions,
    keys: Vec<KeyState>,
    preview: Option<Preview>,
    exit: bool,
    offset: tuinix::Position,
    tmux_client: TmuxClient,
}

impl App {
    pub fn new(layout: Layout, options: AppOptions) -> Result<Self> {
        let mut driver = tuinix::TerminalDriver::new()?;

        driver.enable_mouse_reporting()?;

        let keys = layout
            .keys
            .iter()
            .map(|k| KeyState::new(k.clone()))
            .collect();

        let tmux_client = TmuxClient::new()?;

        let mut app = Self {
            driver,
            input: tuinix::InputDecoder::new(),
            prev_frame: None,
            options,
            keys,
            preview: layout.preview,
            exit: false,
            offset: tuinix::Position::ORIGIN,
            tmux_client,
        };

        app.calculate_offset();

        Ok(app)
    }

    pub fn run(mut self) -> Result<()> {
        self.render()?;

        let mut fds = [
            libc::pollfd {
                fd: self.driver.resize_signal_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.driver.input_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let mut refresh_at = std::time::Instant::now() + self.options.cursor_refresh_interval;

        while !self.exit {
            // While a lone `ESC` byte is held, wait only briefly so it is
            // reported as the Escape key promptly. Otherwise wait until the next
            // cursor refresh so the active pane's cursor keeps blinking.
            let timeout = if self.input.has_uncommitted_escape() {
                ESCAPE_TIMEOUT.min(self.options.cursor_refresh_interval)
            } else {
                self.options.cursor_refresh_interval
            };

            let n = unsafe {
                libc::poll(
                    fds.as_mut_ptr(),
                    fds.len() as libc::nfds_t,
                    timeout_to_millis(timeout),
                )
            };
            if n < 0 {
                let error = std::io::Error::last_os_error();
                // A SIGWINCH handler still makes `poll` return `EINTR`; the
                // resize byte is already in the signal pipe, so retrying reports
                // it as `POLLIN` on the next iteration.
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }

            // A descriptor that hung up or failed can never become ready again.
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0)
            {
                return Err(crate::Error::message("terminal closed"));
            }

            if fds[0].revents & libc::POLLIN != 0 {
                self.driver.handle_resize_signal()?;
                self.calculate_offset();
                self.render()?;
                refresh_at = std::time::Instant::now() + self.options.cursor_refresh_interval;
                continue;
            }

            if fds[1].revents & libc::POLLIN != 0 {
                let mut bytes = [0u8; 1024];
                loop {
                    // The input descriptor is non-blocking, so an empty read
                    // reports `WouldBlock` instead of blocking the loop.
                    match self.driver.read(&mut bytes) {
                        Ok(0) => break,
                        Ok(n) => self.input.feed(&bytes[..n]),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(e) => return Err(e.into()),
                    }
                }
            }

            if n == 0 {
                // The wait elapsed with a lone `ESC` still held, so commit it as
                // the Escape key.
                self.input.commit_escape();
            }

            let mut dirty = false;
            while let Some(input) = self.input.next() {
                self.handle_input(input)?;
                dirty = true;
            }

            if dirty {
                self.render()?;
            }

            if std::time::Instant::now() >= refresh_at {
                // Clicking the active pane resets its cursor blink timer, so the
                // cursor becomes visible again for a while.
                self.tmux_command("select-pane", &["-t", "0:.0"])?;
                self.render()?;
                refresh_at = std::time::Instant::now() + self.options.cursor_refresh_interval;
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, input: tuinix::Input) -> Result<()> {
        match input {
            tuinix::Input::Key(key_input) => {
                self.exit = match key_input.code {
                    tuinix::KeyCode::Char('q') => true,
                    tuinix::KeyCode::Char('c') if key_input.ctrl => true,
                    _ => false,
                };
            }
            tuinix::Input::Mouse(mouse_input) => {
                self.handle_mouse_input(mouse_input)?;
            }
            tuinix::Input::Unrecognized { .. } | tuinix::Input::Paste { .. } => {}
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, mouse_input: tuinix::MouseInput) -> Result<()> {
        if mouse_input.kind != tuinix::MouseInputKind::LeftRelease {
            return Ok(());
        }

        let adjusted_position = tuinix::Position {
            row: mouse_input.position.row.saturating_sub(self.offset.row),
            col: mouse_input.position.col.saturating_sub(self.offset.col),
        };

        let Some(pressed_index) = self
            .keys
            .iter()
            .position(|ks| ks.key.region.contains(adjusted_position))
        else {
            return Ok(());
        };

        if self.keys[pressed_index].key.code.is_modifier() {
            self.handle_modifier_key_pressed(pressed_index)?;
        } else {
            self.handle_normal_key_pressed(pressed_index)?;
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

    fn tmux_command(&mut self, command: &str, args: &[&str]) -> Result<()> {
        self.tmux_client.send_command(command, args)?;
        Ok(())
    }

    fn handle_modifier_key_pressed(&mut self, i: usize) -> Result<()> {
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

    fn handle_normal_key_pressed(&mut self, i: usize) -> Result<()> {
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
        let mut ctrl = false;
        let mut alt = false;
        if code.is_modifiable() {
            if self.is_ctrl_pressed() {
                key_string.push_str("C-");
                ctrl = true;
            }
            if self.is_alt_pressed() {
                key_string.push_str("M-");
                alt = true;
            }
        }
        if self.is_shift_pressed() {
            code = self.keys[i].key.shift_code;
        }

        key_string.push_str(&code.to_string());

        self.tmux_command("send-keys", &["-t", "0:.0", &key_string])?;

        if let Some(preview) = &mut self.preview {
            preview.on_key_sent(code, ctrl, alt);
        }

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
        let terminal_size = self.driver.size();
        let mut actual_frame_size = tuinix::Size::default();

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

        self.offset = tuinix::Position {
            row: offset_row,
            col: offset_col,
        };
    }

    fn render(&mut self) -> Result<()> {
        let terminal_size = self.driver.size();

        if self.options.auto_resize {
            let required_rows = self
                .keys
                .iter()
                .map(|k| k.key.region)
                .chain(self.preview.iter().map(|p| p.region))
                .map(|r| r.position.row + r.size.rows)
                .max()
                .unwrap_or_default();
            if terminal_size.rows != required_rows {
                self.tmux_command(
                    "resize-pane",
                    &["-t", "0:0.1", "-y", &required_rows.to_string()],
                )?;
            }
        }

        let mut frame = tuinix::Frame::new(terminal_size);
        let shift = self.is_shift_active();

        for key_state in &mut self.keys {
            let key_frame = key_state.to_frame(shift);
            frame.put_frame(key_state.key.region.position, &key_frame);
        }

        if let Some(preview) = &self.preview {
            let preview_frame = preview.to_frame();
            frame.put_frame(preview.region.position, &preview_frame);
        }

        // The layout is drawn at its natural size; shift it to the centre of the
        // terminal by pasting it into a terminal-sized frame.
        let mut centered_frame = tuinix::Frame::new(terminal_size);
        centered_frame.put_frame(self.offset, &frame);

        let out = centered_frame.render(self.prev_frame.as_ref(), None);
        self.driver.write_all(&out)?;
        self.driver.flush()?;
        self.prev_frame = Some(centered_frame);

        Ok(())
    }
}

/// Converts a timeout to the millisecond value `libc::poll` expects.
fn timeout_to_millis(timeout: Duration) -> libc::c_int {
    timeout.as_millis().min(libc::c_int::MAX as u128) as libc::c_int
}
