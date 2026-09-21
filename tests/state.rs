//! Properties of the Sans I/O core's transition function.

use std::cell::Cell;

/// A distinct key rectangle in layout coordinates.
fn key_region(row: usize, col: usize) -> tuinix::Region {
    tuinix::Region {
        position: tuinix::Position { row, col },
        size: tuinix::Size { rows: 3, cols: 3 },
    }
}

/// One soft key at `(row, col)`, 3x3 in layout cells.
fn key(code: tuke::layout::KeyCode, row: usize, col: usize) -> tuke::layout::Key {
    tuke::layout::Key {
        code,
        shift_code: code.default_shift_code(),
        region: key_region(row, col),
    }
}

/// A layout with a single column of keys: `Ctrl`, `b`, and `c`.
///
/// The keys are three cells tall each, so the keyboard is nine rows tall and
/// the layout is three columns wide.
fn test_layout() -> tuke::layout::Layout {
    use tuke::layout::KeyCode;
    tuke::layout::Layout {
        keys: vec![
            key(KeyCode::Ctrl, 0, 0),
            key(KeyCode::Char('b'), 3, 0),
            key(KeyCode::Char('c'), 6, 0),
        ],
        preview: None,
    }
}

/// A 40-row, 30-column screen: nine rows of keyboard and a three-column
/// layout centred at column 13.
fn test_size() -> tuinix::Size {
    tuinix::Size { rows: 40, cols: 30 }
}

/// The screen coordinates of the centre of the key at layout cell
/// `(row, col)`, given the state's current keyboard offset.
fn screen_centre(state: &tuke::state::State, row: usize, col: usize) -> tuinix::Position {
    let offset = state.offset();
    let region = key_region(row, col);
    tuinix::Position {
        row: offset.row + region.position.row + region.size.rows / 2,
        col: offset.col + region.position.col + region.size.cols / 2,
    }
}

/// Presses the key at layout cell `(row, col)` by releasing the pointer at its
/// centre, and returns the actions the press asked for.
fn press(state: &mut tuke::state::State, row: usize, col: usize) -> Vec<tuke::action::Action> {
    let position = screen_centre(state, row, col);
    state.update(tuke::event::Event::PointerRelease { position })
}

/// The single key an action list sends, if it sends exactly one.
fn sent_key(actions: &[tuke::action::Action]) -> Option<&termnix::KeyEvent> {
    match actions {
        [
            tuke::action::Action::SendKey(key),
            tuke::action::Action::Redraw,
        ] => Some(key),
        _ => None,
    }
}

#[test]
fn new_state_has_no_modifiers_active() {
    let state = tuke::state::State::new(test_layout(), test_size());

    assert!(!state.is_shift_active());
    assert!(
        state
            .keys()
            .iter()
            .all(|k| k.press == tuke::layout::KeyPressState::Neutral)
    );
}

#[test]
fn new_state_places_the_keyboard_at_the_bottom() {
    let state = tuke::state::State::new(test_layout(), test_size());

    // The layout is 9 rows tall and 3 columns wide, centred in a 30-column
    // screen; the keyboard takes the bottom 9 rows.
    assert_eq!(state.offset(), tuinix::Position { row: 31, col: 13 });
    assert_eq!(state.grid_size(), tuinix::Size { rows: 31, cols: 30 });
}

#[test]
fn resize_to_the_same_size_asks_for_nothing() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    let actions = state.update(tuke::event::Event::Resize { size: test_size() });

    assert!(actions.is_empty());
}

#[test]
fn resize_reports_the_new_grid_and_redraws() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    let actions = state.update(tuke::event::Event::Resize {
        size: tuinix::Size { rows: 50, cols: 40 },
    });

    // 50 rows minus the 9-row keyboard leaves a 41-row grid; the 3-column
    // layout still centres in 40 columns.
    assert_eq!(
        actions,
        vec![
            tuke::action::Action::ResizeSession(tuinix::Size { rows: 41, cols: 40 }),
            tuke::action::Action::Redraw,
        ]
    );
    assert_eq!(state.grid_size().rows, 41);
}

#[test]
fn pointer_release_outside_every_key_asks_for_nothing() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    // Row 15 is above the keyboard (which starts at row 31), so it hits no key.
    let actions = state.update(tuke::event::Event::PointerRelease {
        position: tuinix::Position { row: 15, col: 13 },
    });

    assert!(actions.is_empty());
}

#[test]
fn modifier_key_only_redraws() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    let actions = press(&mut state, 0, 0);

    assert_eq!(actions, vec![tuke::action::Action::Redraw]);
    assert_eq!(
        state.keys()[0].press,
        tuke::layout::KeyPressState::OneshotActivated
    );
}

#[test]
fn normal_key_sends_its_character() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    let actions = press(&mut state, 3, 0);

    let key = sent_key(&actions).expect("a normal key sends exactly one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
    assert_eq!(key.modifiers, termnix::Modifiers::new());
}

#[test]
fn oneshot_ctrl_applies_to_the_next_key_only() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    // Tap Ctrl once, then press `b`: the `b` carries Ctrl and Ctrl returns to
    // neutral, so the following `c` is plain.
    press(&mut state, 0, 0);
    let with_ctrl = press(&mut state, 3, 0);
    let plain = press(&mut state, 6, 0);

    let key = sent_key(&with_ctrl).expect("Ctrl + b sends one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
    assert_eq!(
        key.modifiers,
        termnix::Modifiers {
            ctrl: true,
            ..termnix::Modifiers::new()
        }
    );

    let key = sent_key(&plain).expect("a plain key still sends one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('c'));
    assert_eq!(key.modifiers, termnix::Modifiers::new());
}

#[test]
fn ctrl_tapped_twice_is_held_for_every_key() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    // Two taps arm Ctrl for good, so both keys carry it.
    press(&mut state, 0, 0);
    press(&mut state, 0, 0);
    let first = press(&mut state, 3, 0);
    let second = press(&mut state, 6, 0);

    for actions in [&first, &second] {
        let key = sent_key(actions).expect("Ctrl + key sends one key event");
        assert!(key.modifiers.ctrl, "Ctrl should stay held: {key:?}");
    }
}

#[test]
fn shift_selects_the_shift_label() {
    let mut layout = test_layout();
    layout.keys.push(key(tuke::layout::KeyCode::Shift, 9, 0));
    let mut state = tuke::state::State::new(layout, test_size());

    press(&mut state, 9, 0);
    assert!(state.is_shift_active());

    let actions = press(&mut state, 3, 0);
    let key = sent_key(&actions).expect("Shift + b sends one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('B'));
}

#[test]
fn quit_keys_exit_the_core() {
    for event in [
        tuke::event::Event::Key {
            code: tuinix::KeyCode::Char('q'),
            ctrl: false,
        },
        tuke::event::Event::Key {
            code: tuinix::KeyCode::Char('c'),
            ctrl: true,
        },
    ] {
        let mut state = tuke::state::State::new(test_layout(), test_size());

        let actions = state.update(event);

        assert_eq!(actions, vec![tuke::action::Action::Quit]);
        assert!(state.should_exit());
    }
}

#[test]
fn other_host_keys_do_nothing() {
    let mut state = tuke::state::State::new(test_layout(), test_size());

    let actions = state.update(tuke::event::Event::Key {
        code: tuinix::KeyCode::Char('a'),
        ctrl: false,
    });

    assert!(actions.is_empty());
    assert!(!state.should_exit());
}

#[test]
fn any_screen_size_keeps_the_keyboard_fully_on_screen() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let reached = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let terminal_rows = noprop::sample_usize_in(ctx, 9..=80);
        let terminal_cols = noprop::sample_usize_in(ctx, 3..=120);
        let size = tuinix::Size {
            rows: terminal_rows,
            cols: terminal_cols,
        };
        let mut state = tuke::state::State::new(test_layout(), size);

        // The grid and keyboard partition the screen, so the keyboard's 9
        // rows and 3 columns stay within the terminal at any size.
        let grid = state.grid_size();
        assert_eq!(grid.rows + 9, terminal_rows);
        assert_eq!(grid.cols, terminal_cols);
        let offset = state.offset();
        assert_eq!(offset.row, grid.rows);
        assert!(offset.col + 3 <= terminal_cols);

        // Each key's centre is on screen and maps back to that key, so the
        // offset used by the renderer agrees with the one used by hit-testing.
        for (index, (row, col)) in [(0usize, 0usize), (3, 0), (6, 0)].iter().enumerate() {
            let centre = screen_centre(&state, *row, *col);
            assert!(centre.row < terminal_rows && centre.col < terminal_cols);
            let actions = state.update(tuke::event::Event::PointerRelease { position: centre });
            if index == 0 {
                // Ctrl is a modifier, so its press only redraws.
                assert_eq!(actions, vec![tuke::action::Action::Redraw]);
                reached.set(reached.get() + 1);
            } else {
                assert!(
                    sent_key(&actions).is_some(),
                    "key {index} at its own centre was not sent: {actions:?}"
                );
            }
        }
        // Undo the state changes so the next case starts neutral.
        press(&mut state, 0, 0);
        Ok(())
    })?;

    assert!(
        reached.get() > 0,
        "no case exercised the modifier path\n{runner}"
    );
    Ok(())
}
