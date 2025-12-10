use std::fmt::Write;
use std::time::Duration;
use tuinix::{Terminal, TerminalEvent, TerminalFrame, TerminalInput};

fn main() -> noargs::Result<()> {
    // Initialize terminal
    let mut terminal = Terminal::new()?;

    // Enable mouse input reporting
    terminal.enable_mouse_input()?;

    // Create initial frame
    let mut frame: TerminalFrame = TerminalFrame::new(terminal.size());
    writeln!(frame, "Mouse Handler Demo")?;
    writeln!(frame, "Click anywhere or press 'q' to quit")?;
    terminal.draw(frame)?;

    // Main event loop
    loop {
        match terminal.poll_event(&[], &[], Some(Duration::from_millis(100)))? {
            Some(TerminalEvent::Input(input)) => match input {
                TerminalInput::Key(key_input) => {
                    if let tuinix::KeyCode::Char('q') = key_input.code {
                        break;
                    }
                }
                TerminalInput::Mouse(mouse_input) => {
                    let mut frame: TerminalFrame = TerminalFrame::new(terminal.size());
                    writeln!(frame, "Mouse Event: {:?}", mouse_input.event)?;
                    writeln!(
                        frame,
                        "Position: col={}, row={}",
                        mouse_input.position.col, mouse_input.position.row
                    )?;
                    writeln!(frame, "\nPress 'q' to quit")?;
                    terminal.draw(frame)?;
                }
            },
            Some(TerminalEvent::Resize(size)) => {
                let mut frame: TerminalFrame = TerminalFrame::new(size);
                writeln!(frame, "Terminal resized to {}x{}", size.cols, size.rows)?;
                writeln!(frame, "Press 'q' to quit")?;
                terminal.draw(frame)?;
            }
            _ => {}
        }
    }

    Ok(())
}
