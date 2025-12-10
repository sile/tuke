use std::fmt::Write;

use orfail::OrFail;

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    last_mouse_input: Option<tuinix::MouseInput>,
}

impl App {
    pub fn new() -> orfail::Result<Self> {
        let mut terminal = tuinix::Terminal::new().or_fail()?;
        terminal.enable_mouse_input().or_fail()?;

        Ok(Self {
            terminal,
            last_mouse_input: None,
        })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        self.render().or_fail()?;

        loop {
            match self.terminal.poll_event(&[], &[], None).or_fail()? {
                Some(tuinix::TerminalEvent::Input(input)) => match input {
                    tuinix::TerminalInput::Key(key_input) => {
                        if let tuinix::KeyCode::Char('q') = key_input.code {
                            break;
                        }
                    }
                    tuinix::TerminalInput::Mouse(mouse_input) => {
                        self.last_mouse_input = Some(mouse_input);
                        self.render().or_fail()?;
                    }
                },
                Some(tuinix::TerminalEvent::Resize(_)) => {
                    self.render().or_fail()?;
                }
                _ => {}
            }
        }
        Ok(())
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
        }

        writeln!(frame, "\nPress 'q' to quit").or_fail()?;
        self.terminal.draw(frame).or_fail()?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Button {
    pub label: String,
    pub action: Action,
    pub position: tuinix::TerminalPosition,
    pub size: tuinix::TerminalSize,
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
            position,
            size,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    SendKey { key: tuinix::KeyInput },
}
