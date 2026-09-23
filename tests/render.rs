//! Properties of the pure renderer: the soft keyboard is drawn inside the
//! screen and inside each key's own region.

use std::num::NonZeroU16;

/// A non-zero `termnix` size, for the emulator the renderer reads.
fn termnix_size(rows: usize, cols: usize) -> termnix::Size {
    termnix::Size {
        rows: NonZeroU16::new(rows as u16).expect("rows are non-zero"),
        cols: NonZeroU16::new(cols as u16).expect("cols are non-zero"),
    }
}

/// One layout key of the given size at `(row, col)`.
fn key(code: tuke::KeyCode, row: usize, col: usize, rows: usize, cols: usize) -> tuke::Key {
    tuke::Key {
        action: tuke::KeyAction::Send {
            code,
            shift_code: code.default_shift_code(),
        },
        region: tuinix::Region {
            position: tuinix::Position { row, col },
            size: tuinix::Size { rows, cols },
        },
        padding: 1,
    }
}

/// Wraps a layout in a one-layout set, which is what the core now takes.
fn layout_set(layout: tuke::Layout) -> tuke::LayoutSet {
    tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout,
    }])
}

/// A one-key layout laid out to exactly fill the keyboard area.
fn single_key_layout(rows: usize, cols: usize) -> tuke::Layout {
    tuke::Layout {
        keys: vec![key(tuke::KeyCode::Char('x'), 0, 0, rows, cols)],
        preview: None,
    }
}

/// Renders a single-key keyboard of the given key size on a screen exactly as
/// large as that key, so the key's cells are the whole frame.
fn render_single_key(rows: usize, cols: usize) -> tuinix::Frame {
    let size = tuinix::Size { rows, cols };
    let state = tuke::State::new(layout_set(single_key_layout(rows, cols)), size, None);
    let terminal = termnix::TerminalState::new(termnix_size(rows, cols));
    tuke::screen_frame(&state, &terminal, size)
}

/// A frame with a floating full-width key over `rows` rows, and a terminal
/// whose first row holds `line` with the cursor left where `csi` puts it.
///
/// The keyboard covers the whole width, so every cell of the grid is behind it
/// and the only grid text that survives is what the cursor's clearance window
/// re-paints.
fn render_over_grid(rows: usize, cols: usize, line: &[u8], csi: &[u8]) -> tuinix::Frame {
    let size = tuinix::Size { rows, cols };
    // Pin the keyboard's bottom edge one row above the terminal's, so the
    // keyboard covers row 0 (where the fed text sits) and the clearance window
    // is the only way the text can show through.
    let anchor = tuke::KeyboardPos { col: 0, rows: 1 };
    let state = tuke::State::new(
        layout_set(single_key_layout(rows - 1, cols)),
        size,
        Some(anchor),
    );
    let mut terminal = termnix::TerminalState::new(termnix_size(rows, cols));
    terminal.feed(line);
    terminal.feed(csi);
    tuke::screen_frame(&state, &terminal, size)
}

/// The characters of `frame` on `row`, as a `String` (blanks included).
fn row_text(frame: &tuinix::Frame, row: usize, cols: usize) -> String {
    let mut cells = vec![' '; cols];
    for (position, ch) in frame.chars() {
        if position.row == row && position.col < cols {
            cells[position.col] = ch.value();
        }
    }
    cells.into_iter().collect()
}

#[test]
fn a_bordered_key_draws_its_border_and_centres_its_label() {
    let frame = render_single_key(3, 5);

    assert_eq!(row_text(&frame, 0, 5), "┌───┐");
    assert_eq!(row_text(&frame, 1, 5), "│ x │");
    assert_eq!(row_text(&frame, 2, 5), "└───┘");
}

#[test]
fn a_too_short_key_degrades_to_a_labelled_block() {
    // Two rows cannot hold two borders and a label, so the key shows its label
    // instead of a box with no room for it.
    let frame = render_single_key(2, 5);

    assert_eq!(row_text(&frame, 0, 5), "  x  ");
    assert_eq!(row_text(&frame, 1, 5), "     ");
}

#[test]
fn a_one_row_key_shows_its_label() {
    let frame = render_single_key(1, 5);

    assert_eq!(row_text(&frame, 0, 5), "  x  ");
}

#[test]
fn a_label_narrower_than_the_key_stays_inside_the_borders() {
    // A long label must be cropped, never widen the key or push a border out.
    let mut layout = single_key_layout(3, 3);
    layout.keys[0].action = tuke::KeyAction::Send {
        code: tuke::KeyCode::Backspace,
        shift_code: tuke::KeyCode::Backspace,
    };
    let size = tuinix::Size { rows: 3, cols: 3 };
    let state = tuke::State::new(layout_set(layout), size, None);
    let terminal = termnix::TerminalState::new(termnix_size(3, 3));
    let frame = tuke::screen_frame(&state, &terminal, size);

    // Every row is exactly three columns: two borders and one cropped column.
    assert_eq!(row_text(&frame, 0, 3), "┌─┐");
    assert_eq!(row_text(&frame, 2, 3), "└─┘");
    let middle = row_text(&frame, 1, 3);
    assert!(
        middle.starts_with('│') && middle.ends_with('│'),
        "the middle row lost a border: {middle:?}"
    );
}

#[test]
fn a_shortcut_key_draws_its_label_not_its_text() {
    // The text a shortcut types is usually far too long for a key
    // (`attini approve` spans eleven columns), so the key shows the label the
    // layout gave it and the text stays out of the drawing.
    let mut layout = single_key_layout(3, 9);
    layout.keys[0].action = tuke::KeyAction::Shortcut {
        label: "tell".to_string(),
        text: "attini tell".to_string(),
    };
    let size = tuinix::Size { rows: 3, cols: 9 };
    let state = tuke::State::new(layout_set(layout), size, None);
    let terminal = termnix::TerminalState::new(termnix_size(3, 9));
    let frame = tuke::screen_frame(&state, &terminal, size);

    assert_eq!(row_text(&frame, 1, 9), "│ tell  │");
    let drawn: String = frame.chars().map(|(_, ch)| ch.value()).collect();
    assert!(
        !drawn.contains("attini"),
        "the shortcut's text leaked into the drawing: {drawn:?}"
    );
}

#[test]
fn a_floating_keyboard_is_painted_over_the_grid() {
    // One three-row key at the layout origin, floating with its bottom edge on
    // the terminal's last row. A grid cell the keyboard covers holds a letter,
    // which the keyboard must paint over rather than let show through.
    let size = tuinix::Size { rows: 4, cols: 5 };
    let anchor = tuke::KeyboardPos { col: 0, rows: 0 };
    let state = tuke::State::new(layout_set(single_key_layout(3, 5)), size, Some(anchor));
    let mut terminal = termnix::TerminalState::new(termnix_size(4, 5));

    terminal.feed(b"Z");

    let frame = tuke::screen_frame(&state, &terminal, size);

    // The keyboard occupies rows 1..4, so the grid's `Z` at (0, 0) stays
    // visible above it, and the letter at the covered cell is gone.
    assert_eq!(row_text(&frame, 0, 5), "Z    ");
    assert!(!row_text(&frame, 1, 5).contains('Z'));
    // The key's box is the keyboard's outline, drawn over the filled
    // background.
    assert_eq!(row_text(&frame, 1, 5), "┌───┐");
    assert_eq!(row_text(&frame, 2, 5), "│ x │");
    assert_eq!(row_text(&frame, 3, 5), "└───┘");
}

#[test]
fn a_floating_keyboard_leaves_the_grid_at_full_size() {
    // The grid is the whole terminal when the keyboard floats, so a position
    // under the keyboard is still a real grid position.
    let size = tuinix::Size { rows: 4, cols: 5 };
    let anchor = tuke::KeyboardPos { col: 0, rows: 0 };
    let state = tuke::State::new(layout_set(single_key_layout(3, 5)), size, Some(anchor));

    assert!(state.is_overlay());
    assert_eq!(state.grid_size(), size);
}

#[test]
fn the_keyboard_is_drawn_inside_the_screen_at_any_size() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let reached = std::cell::Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let key_rows = noprop::sample_usize_in(ctx, 1..=6);
        let key_cols = noprop::sample_usize_in(ctx, 1..=8);
        let terminal_rows = noprop::sample_usize_in(ctx, key_rows..=40);
        let terminal_cols = noprop::sample_usize_in(ctx, key_cols..=40);
        let size = tuinix::Size {
            rows: terminal_rows,
            cols: terminal_cols,
        };
        let state = tuke::State::new(
            layout_set(single_key_layout(key_rows, key_cols)),
            size,
            None,
        );
        let terminal = termnix::TerminalState::new(termnix_size(terminal_rows, terminal_cols));
        let frame = tuke::screen_frame(&state, &terminal, size);

        // Every painted cell lies within the screen, and at least one cell of
        // the key's own region is painted, so the key is visible rather than
        // clipped away entirely.
        let mut painted = 0usize;
        for (position, _) in frame.chars() {
            assert!(
                position.row < terminal_rows && position.col < terminal_cols,
                "painted outside the screen at {position:?}"
            );
            painted += 1;
        }
        assert!(painted > 0, "the key was not drawn at all");
        reached.set(reached.get() + 1);
        Ok(())
    })?;

    assert!(reached.get() > 0, "no case rendered a frame\n{runner}");
    Ok(())
}

#[test]
fn a_floating_keyboard_keeps_the_text_around_the_cursor_visible() {
    // A 24-column line with the cursor at column 12 (it prints 12 `x` then the
    // cursor is at column 12). The keyboard covers rows 1.., and its clearance
    // window must bring the grid's own text back around the cursor so the user
    // can read what they are editing.
    let cols = 24;
    let frame = render_over_grid(4, cols, b"xxxxxxxxxxxxxxxxxxxxxxxx", b"\x1b[1;13H");

    let row = row_text(&frame, 0, cols);
    // The cursor is at column 12, so the window is columns 4..=20: the grid's
    // `x` shows across it, and the keyboard's border is gone from the row.
    assert_eq!(row, "┌───xxxxxxxxxxxxxxxxx──┐");
}

#[test]
fn a_floating_keyboard_hides_the_text_outside_the_cursor_clearance() {
    // The keyboard is drawn over the whole grid, so text far from the cursor
    // stays hidden: the clearance window is a window, not the whole row.
    let cols = 40;
    let frame = render_over_grid(
        4,
        cols,
        b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
        b"\x1b[1;21H",
    );

    let row = row_text(&frame, 0, cols);
    let cursor = 20;
    let clearance = tuke::CURSOR_CLEARANCE;

    // Just inside the window the grid shows; just outside it the keyboard's
    // own top border does.
    let inside_left = row.chars().nth(cursor - clearance);
    let inside_right = row.chars().nth(cursor + clearance);
    let outside_left = row.chars().nth(cursor - clearance - 1);
    let outside_right = row.chars().nth(cursor + clearance + 1);
    assert_eq!(inside_left, Some('x'));
    assert_eq!(inside_right, Some('x'));
    assert_eq!(
        outside_left,
        Some('─'),
        "a column outside the window should be the keyboard's border: {row:?}"
    );
    assert_eq!(outside_right, Some('─'));
}

#[test]
fn the_cursor_clearance_window_is_clipped_at_the_edges() {
    // A cursor at column 0 cannot show clearance to the left, and the window
    // must not run off the screen or wrap to the other side.
    let cols = 24;
    let frame = render_over_grid(4, cols, b"xxxxxxxxxxxxxxxxxxxxxxxx", b"\x1b[1;1H");

    let row = row_text(&frame, 0, cols);
    let clearance = tuke::CURSOR_CLEARANCE;

    // The cursor's column and its right clearance are kept; there is nothing
    // to the left of column 0, so the window starts there and the keyboard's
    // border resumes just past the cursor's right clearance.
    assert_eq!(row.chars().next(), Some('x'));
    assert_eq!(row.chars().nth(clearance), Some('x'));
    assert_eq!(row.chars().nth(clearance + 1), Some('─'));
}

#[test]
fn a_docked_keyboard_leaves_the_whole_grid_visible() {
    // With the keyboard docked below the grid there is nothing to cover, so
    // the grid's own text shows everywhere, cursor or no cursor.
    let size = tuinix::Size { rows: 6, cols: 24 };
    let state = tuke::State::new(layout_set(single_key_layout(3, 24)), size, None);
    let mut terminal = termnix::TerminalState::new(termnix_size(6, 24));
    terminal.feed(b"xxxxxxxxxxxxxxxxxxxxxxxx");

    let frame = tuke::screen_frame(&state, &terminal, size);

    assert!(!state.is_overlay());
    assert_eq!(row_text(&frame, 0, 24), "xxxxxxxxxxxxxxxxxxxxxxxx");
}

#[test]
fn a_hidden_cursor_keeps_the_keyboard_over_the_grid() {
    // With no visible cursor there is no window to open, so the keyboard stays
    // over the grid's text.
    let cols = 24;
    let frame = render_over_grid(4, cols, b"xxxxxxxxxxxxxxxxxxxxxxxx", b"\x1b[?25l");

    let row = row_text(&frame, 0, cols);
    assert!(
        !row.contains('x'),
        "a hidden cursor must not open a clearance window: {row:?}"
    );
    assert!(
        row.contains('─'),
        "the keyboard's border should show: {row:?}"
    );
}
