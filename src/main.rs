//! The `tuke` binary: parse arguments and hand the terminal to the I/O edge.

#![deny(unsafe_code)]

mod app;

use std::path::PathBuf;
use std::process::Command;

/// Parses a `COL,ROWS` keyboard position.
///
/// The two numbers are separated by a comma; anything else is an error, so a
/// typo is reported rather than silently read as the origin.
fn parse_keyboard_pos(text: &str) -> tuke::Result<tuke::KeyboardPos> {
    let (col, rows) = text
        .split_once(',')
        .ok_or_else(|| tuke::Error::message(format!("expected COL,ROWS, got {text:?}")))?;
    let col = col
        .trim()
        .parse()
        .map_err(|_| tuke::Error::message(format!("invalid column {col:?}")))?;
    let rows = rows
        .trim()
        .parse()
        .map_err(|_| tuke::Error::message(format!("invalid row {rows:?}")))?;
    Ok(tuke::KeyboardPos { col, rows })
}

fn main() -> noargs::Result<()> {
    let mut args = noargs::raw_args();

    args.metadata_mut().app_name = env!("CARGO_PKG_NAME");
    args.metadata_mut().app_description = env!("CARGO_PKG_DESCRIPTION");

    if noargs::VERSION_FLAG.take(&mut args).is_present() {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    noargs::HELP_FLAG.take_help(&mut args);

    let layout_file_path: Option<PathBuf> = noargs::opt("layout-file")
        .short('l')
        .ty("PATH")
        .env("TUKE_LAYOUT_FILE")
        .doc("Path of layout JSONC file")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    let keyboard_pos: Option<String> = noargs::opt("keyboard-pos")
        .ty("COL,ROWS")
        .doc(
            "Float the keyboard at COL,ROWS (origin: terminal's bottom-left; ROWS \
             counts up from the bottom to the keyboard's bottom edge). Omitted, \
             the keyboard docks to the bottom and the grid takes the rows above it.",
        )
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    let command_line: Option<String> = noargs::opt("command")
        .short('c')
        .ty("COMMAND")
        .doc("Command to run in the session (defaults to $SHELL -l)")
        .take(&mut args)
        .present_and_then(|a| a.value().parse())?;

    let mut command = match command_line {
        Some(line) => {
            let mut command = Command::new("sh");
            command.arg("-c").arg(line);
            command
        }
        None => {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
            let mut command = Command::new(shell);
            command.arg("-l");
            command
        }
    };

    if let Some(help) = args.finish()? {
        print!("{help}");
        return Ok(());
    }

    let keyboard_pos = keyboard_pos
        .map(|pos| parse_keyboard_pos(&pos))
        .transpose()?;

    let layout = layout_file_path
        .map(tuke::Layout::load_from_file)
        .transpose()?
        .unwrap_or_default();
    let app = app::App::new(layout, &mut command, keyboard_pos)?;
    app.run()?;
    Ok(())
}
