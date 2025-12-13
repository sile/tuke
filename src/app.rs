use std::fmt::Write;
use std::process::Command;

use orfail::OrFail;

use crate::config::{Config, KeyState};

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    config: Config,
    last_mouse_input: Option<tuinix::MouseInput>,
    keys: Vec<KeyState>,
    exit: bool,
}

impl App {
    pub fn new(config: Config) -> orfail::Result<Self> {
        let mut terminal = tuinix::Terminal::new().or_fail()?;
        terminal.enable_mouse_input().or_fail()?;

        let keys = config
            .keys
            .iter()
            .map(|k| KeyState::new(k.clone()))
            .collect();

        Ok(Self {
            terminal,
            config,
            last_mouse_input: None,
            keys,
            exit: false,
        })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        self.render().or_fail()?;

        while !self.exit {
            match self.terminal.poll_event(&[], &[], None).or_fail()? {
                Some(tuinix::TerminalEvent::Input(input)) => {
                    self.handle_input(input).or_fail()?;
                    self.render().or_fail()?;
                }
                Some(tuinix::TerminalEvent::Resize(_)) => {
                    self.render().or_fail()?;
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
                self.last_mouse_input = Some(mouse_input);
                self.update_key_states(&mouse_input);

                if mouse_input.event == tuinix::MouseEvent::LeftRelease {
                    if let Some(key_state) = self
                        .keys
                        .iter()
                        .find(|ks| ks.key.region.contains(mouse_input.position))
                    {
                        self.execute_key_press(&key_state.key).or_fail()?;
                    }
                }
            }
        }
        Ok(())
    }

    fn update_key_states(&mut self, mouse_input: &tuinix::MouseInput) {
        for key_state in &mut self.keys {
            key_state.is_pressed = key_state.key.region.contains(mouse_input.position);
        }
    }

    fn execute_key_press(&self, key: &crate::config::Key) -> orfail::Result<()> {
        let key_str = self.key_code_to_string(&key.code);
        Command::new("tmux")
            .args(&["send-keys", "-t", ".0", &key_str])
            .output()
            .or_fail()?;
        Ok(())
    }

    fn key_code_to_string(&self, code: &crate::config::KeyCode) -> String {
        use crate::config::KeyCode;

        match code {
            KeyCode::Char(ch) => ch.to_string(),
            KeyCode::Quit => "q".to_string(),
            KeyCode::Shift => "Shift".to_string(),
            KeyCode::Ctrl => "Ctrl".to_string(),
            KeyCode::Alt => "Alt".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Backspace => "BSpace".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Tab => "Tab".to_string(),
        }
    }

    fn render(&mut self) -> orfail::Result<()> {
        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.terminal.size());

        if let Some(mouse_input) = &self.last_mouse_input {
            writeln!(frame, "Mouse Event: {:?}", mouse_input.event).or_fail()?;
            writeln!(
                frame,
                "Position: col={}, row={}",
                mouse_input.position.col, mouse_input.position.row
            )
            .or_fail()?;

            if let Some(key_state) = self
                .keys
                .iter()
                .find(|ks| ks.key.region.contains(mouse_input.position))
            {
                writeln!(frame, "Pressed Key: {}", key_state.key.code).or_fail()?;
            }
        }

        writeln!(frame, "\nPress 'q' to quit").or_fail()?;

        for key_state in &self.keys {
            self.render_key(&mut frame, key_state).or_fail()?;
        }

        self.terminal.draw(frame).or_fail()?;

        Ok(())
    }

    fn render_key(
        &self,
        frame: &mut tuinix::TerminalFrame,
        key_state: &KeyState,
    ) -> orfail::Result<()> {
        let mut key_frame: tuinix::TerminalFrame =
            tuinix::TerminalFrame::new(key_state.key.region.size);

        let width = key_state.key.region.size.cols;
        let height = key_state.key.region.size.rows;

        let style = if key_state.is_pressed {
            tuinix::TerminalStyle::new().bold()
        } else {
            tuinix::TerminalStyle::new()
        };
        let reset_style = tuinix::TerminalStyle::RESET;

        // Top border
        write!(key_frame, "{}", style).or_fail()?;
        write!(key_frame, "┌").or_fail()?;
        for _ in 1..width - 1 {
            write!(key_frame, "─").or_fail()?;
        }
        writeln!(key_frame, "┐").or_fail()?;

        // Middle rows with left/right borders
        for row in 1..height - 1 {
            write!(key_frame, "│").or_fail()?;
            if row == (height - 1) / 2 {
                let label = key_state.key.code.to_string();
                if label.len() <= width - 2 {
                    let padding = (width - 2 - label.len()) / 2;
                    write!(
                        key_frame,
                        "{:padding$}{}{:padding$}",
                        "",
                        label,
                        "",
                        padding = padding
                    )
                    .or_fail()?;
                } else {
                    write!(key_frame, "{}", &label[..width - 2]).or_fail()?;
                }
            } else {
                write!(key_frame, "{:width$}", "", width = width - 2).or_fail()?;
            }
            writeln!(key_frame, "│").or_fail()?;
        }

        // Bottom border
        write!(key_frame, "└").or_fail()?;
        for _ in 1..width - 1 {
            write!(key_frame, "─").or_fail()?;
        }
        writeln!(key_frame, "┘").or_fail()?;
        write!(key_frame, "{}", reset_style).or_fail()?;

        frame.draw(key_state.key.region.position, &key_frame);

        Ok(())
    }
}
