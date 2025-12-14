use std::process::Command;

use orfail::OrFail;

use crate::config::{Config, KeyState};

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    #[expect(dead_code)]
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
