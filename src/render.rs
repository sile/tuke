//! Pure rendering: turn a [`State`] and the child's terminal state into a
//! [`tuinix::Frame`].
//!
//! Nothing here touches a file descriptor: the edge hands in the sizes and the
//! (I/O-free) [`termnix::TerminalState`] and receives a frame to diff and write.

use crate::KeyState;
use crate::Preview;
use crate::State;

/// Builds the full-screen frame: the child's grid on top, the soft keyboard at
/// the bottom, centred horizontally.
///
/// `terminal_size` is the physical terminal size. The keyboard is drawn at
/// [`State::offset`], and the grid occupies the rows above it, sized by
/// [`State::grid_size`].
pub fn screen_frame(
    state: &State,
    terminal: &termnix::TerminalState,
    terminal_size: tuinix::Size,
) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(terminal_size);

    draw_grid(&mut frame, terminal);

    let shift = state.is_shift_active();
    let offset = state.offset();
    for key_state in state.keys() {
        let key_frame = key_frame(key_state, shift);
        frame.put_frame(
            tuinix::Position {
                row: offset.row + key_state.key.region.position.row,
                col: offset.col + key_state.key.region.position.col,
            },
            &key_frame,
        );
    }

    if let Some(preview) = state.preview() {
        let preview_frame = preview_frame(preview);
        frame.put_frame(
            tuinix::Position {
                row: offset.row + preview.region.position.row,
                col: offset.col + preview.region.position.col,
            },
            &preview_frame,
        );
    }

    frame
}

/// The screen coordinate of the text cursor, if it should be shown.
///
/// The cursor is placed at the child terminal's cursor when it is visible and
/// falls inside the terminal, offset by nothing (the grid starts at the screen
/// origin).
pub fn screen_cursor(
    terminal: &termnix::TerminalState,
    terminal_size: tuinix::Size,
) -> Option<tuinix::Position> {
    if !terminal.modes().cursor_visible {
        return None;
    }
    let pos = terminal.cursor();
    let position = tuinix::Position {
        row: pos.row as usize,
        col: pos.col as usize,
    };
    if position.row < terminal_size.rows && position.col < terminal_size.cols {
        Some(position)
    } else {
        None
    }
}

/// Paints the child terminal's cells into the top-left of `frame`.
fn draw_grid(frame: &mut tuinix::Frame, terminal: &termnix::TerminalState) {
    for (row, cells) in terminal.rows().enumerate() {
        let mut col = 0;
        for cell in cells {
            if cell.width == 0 {
                // The continuation column of a wide glyph; the leading cell
                // already covers it.
                continue;
            }
            let ch = termnix_char(cell);
            if frame.fits(tuinix::Position { row, col }, ch) {
                frame.put_char(tuinix::Position { row, col }, ch);
            }
            col += cell.width as usize;
        }
    }
}

/// Converts one `termnix` cell into a `tuinix` character.
fn termnix_char(cell: &termnix::Cell) -> tuinix::Char {
    let style = termnix_style(cell.style);
    tuinix::Char::new(cell.ch, cell.width as usize, style).expect("cell width is 1 or 2")
}

/// Converts a `termnix` style into a `tuinix` style.
fn termnix_style(style: termnix::Style) -> tuinix::Style {
    let mut out = tuinix::Style::new();
    if style.bold {
        out = out.bold();
    }
    if style.italic {
        out = out.italic();
    }
    if style.underline {
        out = out.underline();
    }
    if style.reverse {
        out = out.reverse();
    }
    if let Some(color) = termnix_color(style.foreground) {
        out = out.fg_color(color);
    }
    if let Some(color) = termnix_color(style.background) {
        out = out.bg_color(color);
    }
    out
}

/// Converts a `termnix` color into a `tuinix` color.
///
/// `termnix::Color::Default` means "whatever the terminal uses", which tuinix
/// represents as `None` (unset), so it maps to `None`.
fn termnix_color(color: termnix::Color) -> Option<tuinix::Color> {
    match color {
        termnix::Color::Default => None,
        termnix::Color::Indexed(index) => Some(tuinix::Color::Indexed(index)),
        termnix::Color::Rgb(r, g, b) => Some(tuinix::Color::Rgb(r, g, b)),
    }
}

/// Renders one soft key as a bordered box of its own region's size.
///
/// `shift` selects the shifted label when the on-screen keyboard has Shift
/// active.
///
/// The box needs at least three rows and three columns: two borders on each
/// axis plus one inner row for the label. A key smaller than that is drawn as
/// a plain labelled block instead, so a cramped layout degrades rather than
/// hiding the label or overwriting its own border. The label is centred in the
/// middle inner row, cropping it rather than overflowing when the key is too
/// narrow.
fn key_frame(key_state: &KeyState, shift: bool) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(key_state.key.region.size);

    let width = key_state.key.region.size.cols;
    let height = key_state.key.region.size.rows;

    let style = match key_state.press {
        crate::layout::KeyPressState::Neutral => tuinix::Style::new(),
        crate::layout::KeyPressState::Pressed => tuinix::Style::new().bold(),
        crate::layout::KeyPressState::Activated => tuinix::Style::new().italic().reverse(),
        crate::layout::KeyPressState::OneshotActivated => tuinix::Style::new().italic(),
    };

    let label = if shift {
        key_state.key.shift_code.to_string()
    } else {
        key_state.key.code.to_string()
    };

    // A box needs a column for each side border and an inner row for the
    // label, besides the top and bottom borders. Below that the label is all
    // that fits, so the key degrades to a filled block rather than to a box
    // that hides its label.
    let inner_cols = width.saturating_sub(2);
    let inner_rows = height.saturating_sub(2);
    if inner_cols == 0 || inner_rows == 0 {
        fill(&mut frame, width, height, &label, style);
        return frame;
    }

    put_text(
        &mut frame,
        tuinix::Position::ORIGIN,
        &format!("┌{}┐", "─".repeat(inner_cols)),
        style,
    );

    // Centre the label in the middle inner row, cropping it to the interior
    // width so a long label never pushes the right border aside.
    let label_row = inner_rows.div_ceil(2);
    for row in 1..=inner_rows {
        // Each row is written from an explicit column 0: `put_char` advances
        // past the right edge of a row by wrapping to the next line, so
        // following the returned position would skip a row.
        let text = if row == label_row {
            format!("│{}│", centred(&label, inner_cols))
        } else {
            format!("│{}│", " ".repeat(inner_cols))
        };
        put_text(&mut frame, tuinix::Position { row, col: 0 }, &text, style);
    }

    put_text(
        &mut frame,
        tuinix::Position {
            row: inner_rows + 1,
            col: 0,
        },
        &format!("└{}┘", "─".repeat(inner_cols)),
        style,
    );

    frame
}

/// Centres `label` in `width` columns, cropping it when it is longer than the
/// space available.
///
/// The result is exactly `width` columns wide, so it never disturbs the
/// borders around it.
fn centred(label: &str, width: usize) -> String {
    let cropped: String = label.chars().take(width).collect();
    let len = cropped.chars().count();
    let padding_left = (width - len) / 2;
    let padding_right = width - len - padding_left;
    format!(
        "{}{cropped}{}",
        " ".repeat(padding_left),
        " ".repeat(padding_right)
    )
}

/// Fills a `width` by `height` frame with `label`, for a key too small to hold
/// a border.
fn fill(frame: &mut tuinix::Frame, width: usize, height: usize, label: &str, style: tuinix::Style) {
    for row in 0..height {
        let text = if row == 0 {
            centred(label, width)
        } else {
            " ".repeat(width)
        };
        put_text(frame, tuinix::Position { row, col: 0 }, &text, style);
    }
}

/// Renders the send preview.
///
/// The preview keeps its own rendering (it is pure layout data), so this just
/// delegates to [`Preview::to_frame`].
fn preview_frame(preview: &Preview) -> tuinix::Frame {
    preview.to_frame()
}

/// Writes `text` into `frame` starting at `at`, advancing one column per
/// character, and returns the position just past the last written character.
///
/// A newline moves to the start of the next row.
fn put_text(
    frame: &mut tuinix::Frame,
    at: tuinix::Position,
    text: &str,
    style: tuinix::Style,
) -> tuinix::Position {
    let mut at = at;
    for c in text.chars() {
        if c == '\n' {
            at = at.next_line();
            continue;
        }
        let ch = tuinix::Char::new(c, 1, style).expect("not a control character");
        at = frame.put_char(at, ch);
    }
    at
}
