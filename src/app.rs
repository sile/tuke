use std::fmt::Write;
use std::process::Command;

use orfail::OrFail;

use crate::config::Config;

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    config: Config,
    last_mouse_input: Option<tuinix::MouseInput>,
    buttons: Vec<Button>,
    exit: bool,
}

impl App {
    pub fn new(config: Config) -> orfail::Result<Self> {
        let mut terminal = tuinix::Terminal::new().or_fail()?;
        terminal.enable_mouse_input().or_fail()?;

        let buttons = vec![
            Button::normal_key(
                "A",
                'a',
                tuinix::TerminalPosition { col: 0, row: 5 },
                tuinix::TerminalSize { cols: 5, rows: 3 },
            ),
            Button::normal_key(
                "S",
                's',
                tuinix::TerminalPosition { col: 6, row: 5 },
                tuinix::TerminalSize { cols: 5, rows: 3 },
            ),
            Button::normal_key(
                "D",
                'd',
                tuinix::TerminalPosition { col: 12, row: 5 },
                tuinix::TerminalSize { cols: 5, rows: 3 },
            ),
            Button::normal_key(
                "F",
                'f',
                tuinix::TerminalPosition { col: 18, row: 6 },
                tuinix::TerminalSize { cols: 7, rows: 4 },
            ),
        ];

        Ok(Self {
            terminal,
            config,
            last_mouse_input: None,
            buttons,
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
                self.update_button_states(&mouse_input);

                if mouse_input.event == tuinix::MouseEvent::LeftRelease
                    && let Some(button) = self
                        .buttons
                        .iter()
                        .find(|btn| btn.region.contains(mouse_input.position))
                {
                    self.execute_action(&button.action).or_fail()?;
                }
            }
        }
        Ok(())
    }

    fn update_button_states(&mut self, mouse_input: &tuinix::MouseInput) {
        for button in &mut self.buttons {
            button.is_pressed = button.region.contains(mouse_input.position);
        }
    }

    fn execute_action(&self, action: &Action) -> orfail::Result<()> {
        match action {
            Action::SendKey { key } => {
                let key_str = self.key_input_to_string(key);
                Command::new("tmux")
                    .args(&["send-keys", "-t", ".0", &key_str])
                    .output()
                    .or_fail()?;
                // Command::new("tmux")
                //    .args(&["select-pane", "-t", ".0"])
                //    .output()
                //    .or_fail()?;
            }
        }
        Ok(())
    }

    fn key_input_to_string(&self, key: &tuinix::KeyInput) -> String {
        let mut result = String::new();

        if key.ctrl {
            result.push_str("C-");
        }
        if key.alt {
            result.push_str("M-");
        }

        match key.code {
            tuinix::KeyCode::Char(ch) => {
                result.push(ch);
            }
            tuinix::KeyCode::Enter => result.push_str("Enter"),
            tuinix::KeyCode::Tab => result.push_str("Tab"),
            tuinix::KeyCode::Backspace => result.push_str("BSpace"),
            tuinix::KeyCode::Delete => result.push_str("Delete"),
            tuinix::KeyCode::Escape => result.push_str("Escape"),
            tuinix::KeyCode::Up => result.push_str("Up"),
            tuinix::KeyCode::Down => result.push_str("Down"),
            tuinix::KeyCode::Left => result.push_str("Left"),
            tuinix::KeyCode::Right => result.push_str("Right"),
            tuinix::KeyCode::PageUp => result.push_str("PageUp"),
            tuinix::KeyCode::PageDown => result.push_str("PageDown"),
            tuinix::KeyCode::Home => result.push_str("Home"),
            tuinix::KeyCode::End => result.push_str("End"),
            _ => {}
        }

        result
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

            // Find which button was clicked using TerminalRegion
            if let Some(button) = self
                .buttons
                .iter()
                .find(|btn| btn.region.contains(mouse_input.position))
            {
                writeln!(frame, "Pressed Button: {}", button.label).or_fail()?;
            }
        }

        writeln!(frame, "\nPress 'q' to quit").or_fail()?;

        for button in &self.buttons {
            button.render(&mut frame).or_fail()?;
        }

        self.terminal.draw(frame).or_fail()?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub region: tuinix::TerminalRegion,
    pub is_pressed: bool,
}

impl Button {
    pub fn normal_key(
        label: &str,
        ch: char,
        position: tuinix::TerminalPosition,
        size: tuinix::TerminalSize,
    ) -> Self {
        Self {
            label: label.to_string(),
            action: Action::SendKey {
                key: tuinix::KeyInput {
                    ctrl: false,
                    alt: false,
                    code: tuinix::KeyCode::Char(ch),
                },
            },
            region: tuinix::TerminalRegion { position, size },
            is_pressed: false,
        }
    }

    pub fn render(&self, frame: &mut tuinix::TerminalFrame) -> orfail::Result<()> {
        let mut button_frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.region.size);

        // Write border chars
        let width = self.region.size.cols;
        let height = self.region.size.rows;

        // Apply style based on pressed state
        let style = if self.is_pressed {
            tuinix::TerminalStyle::new().bold()
        } else {
            tuinix::TerminalStyle::new()
        };
        let reset_style = tuinix::TerminalStyle::RESET;

        // Top border
        write!(button_frame, "{}", style).or_fail()?;
        write!(button_frame, "┌").or_fail()?;
        for _ in 1..width - 1 {
            write!(button_frame, "─").or_fail()?;
        }
        writeln!(button_frame, "┐").or_fail()?;

        // Middle rows with left/right borders
        for row in 1..height - 1 {
            write!(button_frame, "│").or_fail()?;
            if row == (height - 1) / 2 {
                // Center row - write label
                if self.label.len() <= width - 2 {
                    let padding = (width - 2 - self.label.len()) / 2;
                    write!(
                        button_frame,
                        "{:padding$}{}{:padding$}",
                        "",
                        self.label,
                        "",
                        padding = padding
                    )
                    .or_fail()?;
                } else {
                    write!(button_frame, "{}", &self.label[..width - 2]).or_fail()?;
                }
            } else {
                write!(button_frame, "{:width$}", "", width = width - 2).or_fail()?;
            }
            writeln!(button_frame, "│").or_fail()?;
        }

        // Bottom border
        write!(button_frame, "└").or_fail()?;
        for _ in 1..width - 1 {
            write!(button_frame, "─").or_fail()?;
        }
        writeln!(button_frame, "┘").or_fail()?;
        write!(button_frame, "{}", reset_style).or_fail()?;

        frame.draw(self.region.position, &button_frame);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    // Send the key to the previous pane using `tmux send-keys` command
    SendKey { key: tuinix::KeyInput },
    // SendKeys
    // SelectPane
    // SelectLayer{ctrl|alt|shift|custom}
}
