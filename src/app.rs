use std::process::Command;

use orfail::OrFail;

use crate::config::{Config, KeyCode, KeyPressState, KeyState};

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    #[expect(dead_code)]
    config: Config,
    keys: Vec<KeyState>,
    pane_index: usize,
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
            keys,
            pane_index: 0,
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
                self.handle_mouse_input(mouse_input).or_fail()?;
            }
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, mouse_input: tuinix::MouseInput) -> orfail::Result<()> {
        if mouse_input.event != tuinix::MouseEvent::LeftRelease {
            return Ok(());
        }

        let Some(pressed_index) = self
            .keys
            .iter()
            .position(|ks| ks.key.region.contains(mouse_input.position))
        else {
            return Ok(());
        };

        if self.keys[pressed_index].key.code.is_modifier() {
            self.handle_modifier_key_pressed(pressed_index).or_fail()?;
        } else {
            self.handle_normal_key_pressed(pressed_index).or_fail()?;
        }

        Ok(())
    }

    fn handle_modifier_key_pressed(&mut self, _i: usize) -> orfail::Result<()> {
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

        let mut key_string = String::new();
        if self.is_ctrl_pressed() {
            key_string.push_str("C-");
        }
        if self.is_alt_pressed() {
            key_string.push_str("M-");
        }
        if self.is_shift_pressed() {
            key_string.push_str("S-");
        }
        key_string.push_str(&self.keys[i].key.code.to_string());

        Command::new("tmux")
            .args(&[
                "send-keys",
                "-t",
                &format!(".{}", self.pane_index),
                &key_string,
            ])
            .output()
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

    fn render(&mut self) -> orfail::Result<()> {
        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.terminal.size());

        for key_state in &self.keys {
            frame.draw(
                key_state.key.region.position,
                &key_state.to_frame().or_fail()?,
            );
        }

        self.terminal.draw(frame).or_fail()?;
        Ok(())
    }
}
