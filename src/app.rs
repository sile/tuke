use std::fmt::Write;
use std::time::Duration;

use orfail::OrFail;

#[derive(Debug)]
pub struct App {
    terminal: tuinix::Terminal,
}

impl App {
    pub fn new() -> orfail::Result<Self> {
        let mut terminal = tuinix::Terminal::new().or_fail()?;
        terminal.enable_mouse_input().or_fail()?;

        Ok(Self { terminal })
    }

    pub fn run(mut self) -> orfail::Result<()> {
        // Create initial frame
        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.terminal.size());
        writeln!(frame, "Mouse Handler Demo").or_fail()?;
        writeln!(frame, "Click anywhere or press 'q' to quit").or_fail()?;
        self.terminal.draw(frame).or_fail()?;

        // Main event loop
        loop {
            match self
                .terminal
                .poll_event(&[], &[], Some(Duration::from_millis(100)))
                .or_fail()?
            {
                Some(tuinix::TerminalEvent::Input(input)) => match input {
                    tuinix::TerminalInput::Key(key_input) => {
                        if let tuinix::KeyCode::Char('q') = key_input.code {
                            break;
                        }
                    }
                    tuinix::TerminalInput::Mouse(mouse_input) => {
                        let mut frame: tuinix::TerminalFrame =
                            tuinix::TerminalFrame::new(self.terminal.size());
                        writeln!(frame, "Mouse Event: {:?}", mouse_input.event).or_fail()?;
                        writeln!(
                            frame,
                            "Position: col={}, row={}",
                            mouse_input.position.col, mouse_input.position.row
                        )
                        .or_fail()?;
                        writeln!(frame, "\nPress 'q' to quit").or_fail()?;
                        self.terminal.draw(frame).or_fail()?;
                    }
                },
                Some(tuinix::TerminalEvent::Resize(size)) => {
                    let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(size);
                    writeln!(frame, "Terminal resized to {}x{}", size.cols, size.rows).or_fail()?;
                    writeln!(frame, "Press 'q' to quit").or_fail()?;
                    self.terminal.draw(frame).or_fail()?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}
