use std::fmt::Write;

use orfail::OrFail;

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
    last_mouse_input: Option<tuinix::MouseInput>,
    buttons: Vec<Button>,
}

impl App {
    pub fn new() -> orfail::Result<Self> {
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
                tuinix::TerminalPosition { col: 18, row: 5 },
                tuinix::TerminalSize { cols: 5, rows: 3 },
            ),
        ];

        Ok(Self {
            terminal,
            last_mouse_input: None,
            buttons,
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

    pub fn render(&self, frame: &mut tuinix::TerminalFrame) -> orfail::Result<()> {
        let mut button_frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.size);

        // Write border chars
        let width = self.size.cols;
        let height = self.size.rows;

        // Top border
        write!(button_frame, "┌").or_fail()?;
        for _ in 1..width - 1 {
            write!(button_frame, "─").or_fail()?;
        }
        write!(button_frame, "┐").or_fail()?;

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
            write!(button_frame, "│").or_fail()?;
        }

        // Bottom border
        write!(button_frame, "└").or_fail()?;
        for _ in 1..width - 1 {
            write!(button_frame, "─").or_fail()?;
        }
        write!(button_frame, "┘").or_fail()?;

        frame.draw(self.position, &button_frame);

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    SendKey { key: tuinix::KeyInput },
}
