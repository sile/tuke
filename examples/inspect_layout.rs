//! Reads a layout JSONC file and prints what tuke sees in it.
//!
//! Run it against a file you are writing, or against a shipped layout:
//!
//! ```console
//! $ cargo run --example inspect_layout layouts/mini.jsonc
//! ```
//!
//! It prints one line per key (code, region, label) and then the summary a
//! layout chooser would need: the keyboard's extent in columns and rows. Both
//! come out of the same public API the binary uses, so this is also a worked
//! example of reading a layout from Rust.

use std::path::PathBuf;
use std::process::ExitCode;

use tuke::{Key, KeyAction, Layout};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: inspect_layout <layout.jsonc>");
        return ExitCode::FAILURE;
    };

    let layout = match Layout::load_from_file(&path) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    println!("{}: {} keys", path.display(), layout.keys.len());
    for key in &layout.keys {
        let region = key.region;
        println!(
            "  {:<7} row={} col={} {}x{}  label={:?}{}",
            code_text(key),
            region.position.row,
            region.position.col,
            region.size.cols,
            region.size.rows,
            label(key),
            shift_text(key),
        );
    }

    let (cols, rows) = extent(&layout);
    println!("extent: {cols} cols x {rows} rows");
    if let Some(preview) = &layout.preview {
        println!(
            "preview: row={} col={} {}x{}",
            preview.region.position.row,
            preview.region.position.col,
            preview.region.size.cols,
            preview.region.size.rows,
        );
    }

    ExitCode::SUCCESS
}

/// The label the renderer would draw for a key: its code as text, except that
/// a space is shown as `Space` so it is visible in the listing.
///
/// A switch key has no code, so it is labelled with the layout it shows.
fn label(key: &Key) -> String {
    match &key.action {
        KeyAction::Send { code, .. } => match code.to_string().as_str() {
            " " => "Space".to_owned(),
            other => other.to_owned(),
        },
        KeyAction::Switch { to } => format!("-> {to}"),
    }
}

/// The text a send key carries, or `(switch)` for a key that switches layouts.
fn code_text(key: &Key) -> String {
    match key.action {
        KeyAction::Send { code, .. } => code.to_string(),
        KeyAction::Switch { .. } => "(switch)".to_owned(),
    }
}

/// The extra column noting a shifted code, or nothing when Shift is the same
/// as the unshifted code or the key switches layouts.
fn shift_text(key: &Key) -> String {
    match key.action {
        KeyAction::Send { code, shift_code } if shift_code != code => {
            format!(" shift={:?}", shift_code.to_string())
        }
        _ => String::new(),
    }
}

/// The keyboard's extent in layout cells: the largest right and bottom edge
/// over every key and the preview.
///
/// This is exactly what a layout chooser compares against the terminal size:
/// the columns must fit the terminal, and the rows are the keyboard's claim on
/// the bottom of the screen.
fn extent(layout: &Layout) -> (usize, usize) {
    let regions = layout
        .keys
        .iter()
        .map(|key| key.region)
        .chain(layout.preview.iter().map(|preview| preview.region));

    regions.fold((0, 0), |(cols, rows), region| {
        (
            cols.max(region.position.col + region.size.cols),
            rows.max(region.position.row + region.size.rows),
        )
    })
}
