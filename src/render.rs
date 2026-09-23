//! Pure rendering: turn a [`State`] and the child's terminal state into a
//! [`tuinix::Frame`].
//!
//! Nothing here touches a file descriptor: the edge hands in the sizes and the
//! (I/O-free) [`termnix::TerminalState`] and receives a frame to diff and write.

use crate::KeyState;
use crate::Preview;
use crate::State;

/// Builds the full-screen frame.
///
/// The grid is painted first, over the whole terminal, and the keyboard is
/// then painted *over* it.
///
/// The keyboard is not just its keys: the area of its bounding box is filled
/// in and outlined first, so the grid does not show through the gaps between
/// keys. The keys and the preview are pasted on top of that, and finally the
/// border is drawn around the whole keyboard.
///
/// `terminal_size` is the physical terminal size.
///
/// Finally, when the keyboard covers the cursor's row, the grid is re-painted
/// over a window of columns to either side of the cursor, so the text the user
/// is editing shows through the keyboard. See [`CURSOR_CLEARANCE`].
pub fn screen_frame(
    state: &State,
    terminal: &termnix::TerminalState,
    terminal_size: tuinix::Size,
) -> tuinix::Frame {
    let mut frame = tuinix::Frame::new(terminal_size);

    draw_grid(
        &mut frame,
        terminal,
        0..terminal_size.rows,
        0..terminal_size.cols,
    );

    let shift = state.is_shift_active();
    let offset = state.offset();

    draw_keyboard_background(&mut frame, state, offset);

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

    // The keyboard is drawn over the whole grid, so on the row where the
    // cursor sits it can cover the text being edited. Paint that row back over
    // a window around the cursor.
    if let Some(cursor) = visible_cursor(terminal) {
        let columns = cursor_clearance_columns(cursor.col, terminal_size.cols);
        draw_grid(&mut frame, terminal, cursor.row..cursor.row + 1, columns);
    }

    frame
}

/// How many columns of the grid's own text are kept visible on each side of
/// the cursor while the keyboard floats over it.
///
/// A cursor alone is not enough to read what is being edited: the word around
/// it is what tells the user where they are. Eight columns on each side is a
/// wide enough window to hold a typical word's tail and head without opening a
/// hole in the keyboard the size of the whole row.
pub const CURSOR_CLEARANCE: usize = 8;

/// The columns of a cursor's row to keep clear of the keyboard: the cursor's
/// own column plus [`CURSOR_CLEARANCE`] columns to either side, clipped to a
/// `terminal_cols`-wide terminal.
///
/// The cursor column is always included, so a cursor near an edge still shows
/// even when its clearance is clipped away.
fn cursor_clearance_columns(cursor_col: usize, terminal_cols: usize) -> std::ops::Range<usize> {
    let start = cursor_col.saturating_sub(CURSOR_CLEARANCE);
    let end = (cursor_col + CURSOR_CLEARANCE + 1).min(terminal_cols);
    start..end
}

/// Fills the keyboard's bounding box with blanks and outlines it, so the grid
/// painted underneath does not show through the keyboard.
///
/// `offset` is the layout's top-left in screen coordinates and the box is
/// `layout_size` cells. Anything past the terminal's edge is left out, so a
/// floating keyboard partly off-screen is cropped rather than wrapping.
fn draw_keyboard_background(frame: &mut tuinix::Frame, state: &State, offset: tuinix::Position) {
    let size = state.layout_size();
    let blank = tuinix::Char::new(' ', 1, tuinix::Style::new()).expect("a space is one column");
    for row in 0..size.rows {
        for col in 0..size.cols {
            let at = tuinix::Position {
                row: offset.row + row,
                col: offset.col + col,
            };
            if frame.fits(at, blank) {
                frame.put_char(at, blank);
            }
        }
    }

    draw_keyboard_border(frame, offset, size);
}

/// Draws the one-cell border around the keyboard's bounding box.
///
/// Every edge cell is chosen from the box's own size, so a one-row or
/// one-column keyboard still gets a closed outline (a single line rather than
/// overlapping corners). Only cells that fit in the terminal are drawn, so a
/// box partly off-screen keeps the border it can show.
fn draw_keyboard_border(frame: &mut tuinix::Frame, offset: tuinix::Position, size: tuinix::Size) {
    let style = tuinix::Style::new();
    let last_col = size.cols.saturating_sub(1);
    let last_row = size.rows.saturating_sub(1);
    for row in 0..size.rows {
        for col in 0..size.cols {
            let ch = match (row == 0, row == last_row, col == 0, col == last_col) {
                (true, false, true, false) => '┌',
                (true, false, false, true) => '┐',
                (false, true, true, false) => '└',
                (false, true, false, true) => '┘',
                (true, false, _, _) | (false, true, _, _) => '─',
                (_, _, true, false) | (_, _, false, true) => '│',
                _ => continue,
            };
            let ch = tuinix::Char::new(ch, 1, style).expect("a border glyph is one column");
            let at = tuinix::Position {
                row: offset.row + row,
                col: offset.col + col,
            };
            if frame.fits(at, ch) {
                frame.put_char(at, ch);
            }
        }
    }
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
    let position = visible_cursor(terminal)?;
    if position.row < terminal_size.rows && position.col < terminal_size.cols {
        Some(position)
    } else {
        None
    }
}

/// The child terminal's cursor in screen coordinates, when the child has the
/// cursor visible.
///
/// Unlike [`screen_cursor`], the position is not checked against the terminal
/// size: a caller that draws by clipping (such as [`screen_frame`]) wants the
/// cursor even when it sits just past the edge, so the clearance window around
/// it can still be computed.
fn visible_cursor(terminal: &termnix::TerminalState) -> Option<tuinix::Position> {
    if !terminal.modes().cursor_visible {
        return None;
    }
    let pos = terminal.cursor();
    Some(tuinix::Position {
        row: pos.row as usize,
        col: pos.col as usize,
    })
}

/// Paints the child terminal's cells into the top-left of `frame`.
///
/// Only the cells in the `rows` and `cols` windows are considered, so a caller
/// can paint one row (or part of one) again over something drawn on top of it.
/// The column cursor still walks the whole row, so a wide glyph that straddles
/// the window's left edge is skipped rather than truncated into it.
fn draw_grid(
    frame: &mut tuinix::Frame,
    terminal: &termnix::TerminalState,
    rows: std::ops::Range<usize>,
    cols: std::ops::Range<usize>,
) {
    for (row, cells) in terminal.rows().enumerate() {
        if !rows.contains(&row) {
            continue;
        }
        let mut col = 0;
        for cell in cells {
            if cell.width == 0 {
                // The continuation column of a wide glyph; the leading cell
                // already covers it.
                continue;
            }
            let width = cell.width as usize;
            if cols.contains(&col) {
                let ch = termnix_char(cell);
                if frame.fits(tuinix::Position { row, col }, ch) {
                    frame.put_char(tuinix::Position { row, col }, ch);
                }
            }
            col += width;
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

    let label = match &key_state.key.action {
        crate::layout::KeyAction::Send { code, shift_code } => {
            if shift {
                shift_code.to_string()
            } else {
                code.to_string()
            }
        }
        // A switch key is labelled with where it goes: the layout it shows is
        // the only thing a press on it can mean, so its name is what the user
        // needs to see.
        crate::layout::KeyAction::Switch { to } => to.clone(),
        // A shortcut key is labelled with its own label: the text it types is
        // usually too long to draw, and the label is what the user named it.
        crate::layout::KeyAction::Shortcut { label, .. } => label.clone(),
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
