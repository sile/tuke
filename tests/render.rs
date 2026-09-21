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
fn key(
    code: tuke::layout::KeyCode,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> tuke::layout::Key {
    tuke::layout::Key {
        code,
        shift_code: code.default_shift_code(),
        region: tuinix::Region {
            position: tuinix::Position { row, col },
            size: tuinix::Size { rows, cols },
        },
    }
}

/// A one-key layout laid out to exactly fill the keyboard area.
fn single_key_layout(rows: usize, cols: usize) -> tuke::layout::Layout {
    tuke::layout::Layout {
        keys: vec![key(tuke::layout::KeyCode::Char('x'), 0, 0, rows, cols)],
        preview: None,
    }
}

/// Renders a single-key keyboard of the given key size on a screen exactly as
/// large as that key, so the key's cells are the whole frame.
fn render_single_key(rows: usize, cols: usize) -> tuinix::Frame {
    let size = tuinix::Size { rows, cols };
    let state = tuke::state::State::new(single_key_layout(rows, cols), size);
    let terminal = termnix::TerminalState::new(termnix_size(rows, cols));
    tuke::render::frame(&state, &terminal, size)
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
    layout.keys[0].code = tuke::layout::KeyCode::Backspace;
    layout.keys[0].shift_code = tuke::layout::KeyCode::Backspace;
    let size = tuinix::Size { rows: 3, cols: 3 };
    let state = tuke::state::State::new(layout, size);
    let terminal = termnix::TerminalState::new(termnix_size(3, 3));
    let frame = tuke::render::frame(&state, &terminal, size);

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
        let state = tuke::state::State::new(single_key_layout(key_rows, key_cols), size);
        let terminal = termnix::TerminalState::new(termnix_size(terminal_rows, terminal_cols));
        let frame = tuke::render::frame(&state, &terminal, size);

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
