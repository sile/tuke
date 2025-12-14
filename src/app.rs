use std::process::Command;

use orfail::OrFail;

use crate::config::{Config, Key, KeyState};

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

    fn handle_modifier_key_pressed(&mut self, i: usize) -> orfail::Result<()> {
        Ok(())
    }

    fn handle_normal_key_pressed(&mut self, i: usize) -> orfail::Result<()> {
        for key in &mut self.keys {
            if key.is_modifier_active {
                continue;
            }
            key.is_pressed = false;
        }
        self.keys[i].is_pressed = true;

        Ok(())
    }

    /* TODO: remove
        fn handle_key_press(&self, key: &Key) -> orfail::Result<()> {
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
    */

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
