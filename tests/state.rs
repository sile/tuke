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
fn key(code: tuke::KeyCode, row: usize, col: usize) -> tuke::Key {
    tuke::Key {
        action: tuke::KeyAction::Send {
            code,
            shift_code: code.default_shift_code(),
        },
        region: key_region(row, col),
        padding: 1,
    }
}

/// Wraps a layout in a one-layout set named `default`, which is what the core
/// takes now that a key can switch layouts.
fn layout_set(layout: tuke::Layout) -> tuke::LayoutSet {
    tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout,
    }])
}

/// A layout with a single column of keys: `Ctrl`, `b`, and `c`.
///
/// The keys are three cells tall each, so the keyboard is nine rows tall and
/// the layout is three columns wide.
fn test_layout() -> tuke::Layout {
    tuke::Layout {
        keys: vec![
            key(tuke::KeyCode::Ctrl, 0, 0),
            key(tuke::KeyCode::Char('b'), 3, 0),
            key(tuke::KeyCode::Char('c'), 6, 0),
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
fn screen_centre(state: &tuke::State, row: usize, col: usize) -> tuinix::Position {
    let offset = state.offset();
    let region = key_region(row, col);
    tuinix::Position {
        row: offset.row + region.position.row + region.size.rows / 2,
        col: offset.col + region.position.col + region.size.cols / 2,
    }
}

/// Presses the key at layout cell `(row, col)` by releasing the pointer at its
/// centre, and returns the actions the press asked for.
fn press(state: &mut tuke::State, row: usize, col: usize) -> Vec<tuke::Action> {
    let position = screen_centre(state, row, col);
    state.update(tuke::Event::PointerRelease { position })
}

/// The single key an action list sends, if it sends exactly one.
fn sent_key(actions: &[tuke::Action]) -> Option<&termnix::KeyEvent> {
    match actions {
        [tuke::Action::SendKey(key), tuke::Action::Redraw] => Some(key),
        _ => None,
    }
}

/// The code of the state's soft key at index `index`.
///
/// These tests only build send keys when they read a code back, so a key of
/// any other kind is a mistake in the test rather than a case to handle.
fn code_of_state_key(state: &tuke::State, index: usize) -> tuke::KeyCode {
    match state.keys()[index].key.action {
        tuke::KeyAction::Send { code, .. } => code,
        tuke::KeyAction::Switch { .. } => panic!("expected a send key, got a switch key"),
        tuke::KeyAction::Shortcut { .. } => panic!("expected a send key, got a shortcut key"),
    }
}

#[test]
fn new_state_has_no_modifiers_active() {
    let state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    assert!(!state.is_shift_active());
    assert!(
        state
            .keys()
            .iter()
            .all(|k| k.press == tuke::KeyPressState::Neutral)
    );
}

#[test]
fn new_state_places_the_keyboard_at_the_bottom() {
    let state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // The layout is 9 rows tall and 3 columns wide, centred in a 30-column
    // screen; the keyboard takes the bottom 9 rows.
    assert_eq!(state.offset(), tuinix::Position { row: 31, col: 13 });
    assert_eq!(state.grid_size(), tuinix::Size { rows: 31, cols: 30 });
}

/// A two-layout set: `default` has a switch key that shows `other`, and
/// `other` has a single `z` key.
fn switch_set() -> tuke::LayoutSet {
    tuke::LayoutSet::from_named(vec![
        tuke::NamedLayout {
            name: "default".to_string(),
            layout: tuke::Layout {
                keys: vec![tuke::Key {
                    action: tuke::KeyAction::Switch {
                        to: "other".to_string(),
                    },
                    region: key_region(0, 0),
                    padding: 1,
                }],
                preview: None,
            },
        },
        tuke::NamedLayout {
            name: "other".to_string(),
            layout: tuke::Layout {
                keys: vec![key(tuke::KeyCode::Char('z'), 0, 0)],
                preview: None,
            },
        },
    ])
}

#[test]
fn the_state_starts_on_the_first_layout() {
    let state = tuke::State::new(switch_set(), test_size(), None);

    assert_eq!(state.current_layout_name(), "default");
}

#[test]
fn pressing_a_switch_key_shows_its_target() {
    let mut state = tuke::State::new(switch_set(), test_size(), None);

    let actions = press(&mut state, 0, 0);

    // The switch is the whole effect: it asks for a repaint and nothing else,
    // because no key was pressed on the child's behalf.
    assert_eq!(actions, vec![tuke::Action::Redraw]);
    assert_eq!(state.current_layout_name(), "other");
    assert_eq!(code_of_state_key(&state, 0), tuke::KeyCode::Char('z'));
}

#[test]
fn a_switched_in_key_sends_its_own_code() {
    let mut state = tuke::State::new(switch_set(), test_size(), None);

    press(&mut state, 0, 0);
    let actions = press(&mut state, 0, 0);

    let key = sent_key(&actions).expect("the new layout's key is sent");
    assert_eq!(key.code, termnix::KeyCode::Char('z'));
}

#[test]
fn a_switch_to_an_unknown_layout_does_nothing() {
    let set = tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "only".to_string(),
        layout: tuke::Layout {
            keys: vec![tuke::Key {
                action: tuke::KeyAction::Switch {
                    to: "missing".to_string(),
                },
                region: key_region(0, 0),
                padding: 1,
            }],
            preview: None,
        },
    }]);
    let mut state = tuke::State::new(set, test_size(), None);

    let actions = press(&mut state, 0, 0);

    assert!(actions.is_empty(), "expected no actions, got {actions:?}");
    assert_eq!(state.current_layout_name(), "only");
}

/// A layout whose only key is a shortcut around `text`, labelled `label`.
fn shortcut_layout(label: &str, text: &str) -> tuke::Layout {
    tuke::Layout {
        keys: vec![tuke::Key {
            action: tuke::KeyAction::Shortcut {
                label: label.to_string(),
                text: text.to_string(),
            },
            region: key_region(0, 0),
            padding: 1,
        }],
        preview: None,
    }
}

#[test]
fn pressing_a_shortcut_key_asks_for_its_text() {
    let set = tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout: shortcut_layout("tell", "attini tell"),
    }]);
    let mut state = tuke::State::new(set, test_size(), None);

    let actions = press(&mut state, 0, 0);

    // The text travels whole, with no Enter: the user presses Enter
    // themselves once they have read the line back.
    assert_eq!(
        actions,
        vec![
            tuke::Action::SendShortcut("attini tell".to_string()),
            tuke::Action::Redraw
        ]
    );
}

#[test]
fn a_shortcut_is_not_held_down() {
    // A shortcut types a string, so it has no press state to keep: after the
    // press the key is neutral again and a second press types it again rather
    // than being treated as a repeat of a held key.
    let set = tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout: shortcut_layout("tell", "attini tell"),
    }]);
    let mut state = tuke::State::new(set, test_size(), None);

    press(&mut state, 0, 0);
    assert_eq!(
        state.keys()[0].press,
        tuke::KeyPressState::Neutral,
        "a shortcut key should not stay pressed"
    );

    let actions = press(&mut state, 0, 0);
    assert_eq!(
        actions,
        vec![
            tuke::Action::SendShortcut("attini tell".to_string()),
            tuke::Action::Redraw
        ],
        "a second press types the same text again"
    );
}

#[test]
fn a_shortcut_is_the_same_text_whatever_the_modifiers_hold() {
    // The text does not depend on the modifier keys the way a single code does
    // (`C-a` is not `Ctrl` applied to a string), so an armed Ctrl neither
    // changes what a shortcut types nor is consumed by it. The armed Ctrl is
    // observed through its effect on the next ordinary key, which is the only
    // way it becomes visible: it still carries Ctrl.
    let set = tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout: tuke::Layout {
            keys: vec![
                key(tuke::KeyCode::Ctrl, 0, 0),
                tuke::Key {
                    action: tuke::KeyAction::Shortcut {
                        label: "tell".to_string(),
                        text: "attini tell".to_string(),
                    },
                    region: key_region(3, 0),
                    padding: 1,
                },
                key(tuke::KeyCode::Char('b'), 6, 0),
            ],
            preview: None,
        },
    }]);
    let mut state = tuke::State::new(set, test_size(), None);

    press(&mut state, 0, 0); // arm Ctrl
    let actions = press(&mut state, 3, 0);

    assert_eq!(
        actions,
        vec![
            tuke::Action::SendShortcut("attini tell".to_string()),
            tuke::Action::Redraw
        ]
    );

    let actions = press(&mut state, 6, 0);
    let key = sent_key(&actions).expect("the key after the shortcut is sent");
    assert!(
        key.modifiers.ctrl,
        "Ctrl leaked past the shortcut key: {key:?}"
    );
}

#[test]
fn a_shortcut_that_types_nothing_is_still_a_press() {
    // The text is the user's, so an empty one is a layout that spells nothing.
    // It is carried through rather than being dropped in the core, because
    // there is no key press to send and no marker to add either way.
    let set = tuke::LayoutSet::from_named(vec![tuke::NamedLayout {
        name: "default".to_string(),
        layout: shortcut_layout("none", ""),
    }]);
    let mut state = tuke::State::new(set, test_size(), None);

    let actions = press(&mut state, 0, 0);

    assert_eq!(
        actions,
        vec![
            tuke::Action::SendShortcut(String::new()),
            tuke::Action::Redraw
        ]
    );
}

#[test]
fn a_switch_that_changes_the_height_keeps_the_bottom_edge() {
    // The keyboard is anchored by its bottom edge, so a taller board grows
    // upward: the bottom of the screen stays the bottom of the screen.
    let set = tuke::LayoutSet::from_named(vec![
        tuke::NamedLayout {
            name: "default".to_string(),
            layout: tuke::Layout {
                keys: vec![tuke::Key {
                    action: tuke::KeyAction::Switch {
                        to: "tall".to_string(),
                    },
                    region: key_region(0, 0),
                    padding: 1,
                }],
                preview: None,
            },
        },
        tuke::NamedLayout {
            name: "tall".to_string(),
            layout: tuke::Layout {
                keys: vec![
                    key(tuke::KeyCode::Char('b'), 0, 0),
                    key(tuke::KeyCode::Char('c'), 3, 0),
                ],
                preview: None,
            },
        },
    ]);
    let mut state = tuke::State::new(set, test_size(), None);

    press(&mut state, 0, 0);

    // The taller board is 6 rows instead of 3, so the grid shrinks by 3 and
    // the keyboard's top edge moves up: its bottom row is still 39.
    assert_eq!(state.current_layout_name(), "tall");
    let offset = state.offset();
    let layout_rows = state.layout_size().rows;
    assert_eq!(
        offset.row + layout_rows,
        test_size().rows,
        "the keyboard's bottom edge left the bottom of the screen"
    );
    assert_eq!(state.grid_size().rows + layout_rows, test_size().rows);
}

#[test]
fn a_switch_that_changes_the_grid_size_asks_the_session_to_resize() {
    // The PTY has to be told too: the grid area shrank, so the child would
    // otherwise paint into rows the keyboard covers.
    let set = tuke::LayoutSet::from_named(vec![
        tuke::NamedLayout {
            name: "default".to_string(),
            layout: tuke::Layout {
                keys: vec![tuke::Key {
                    action: tuke::KeyAction::Switch {
                        to: "tall".to_string(),
                    },
                    region: key_region(0, 0),
                    padding: 1,
                }],
                preview: None,
            },
        },
        tuke::NamedLayout {
            name: "tall".to_string(),
            layout: tuke::Layout {
                keys: vec![
                    key(tuke::KeyCode::Char('b'), 0, 0),
                    key(tuke::KeyCode::Char('c'), 3, 0),
                ],
                preview: None,
            },
        },
    ]);
    let mut state = tuke::State::new(set, test_size(), None);

    let actions = press(&mut state, 0, 0);

    assert_eq!(
        actions,
        vec![
            tuke::Action::Redraw,
            tuke::Action::ResizeSession(state.grid_size()),
        ],
        "the switch changed the grid size, so the session must be resized"
    );
}

#[test]
fn a_switch_can_leave_the_grid_size_alone() {
    // Boards of the same size need no resize: the grid is unchanged, so the
    // press asks only for the repaint the new layout needs.
    let mut state = tuke::State::new(switch_set(), test_size(), None);

    let actions = press(&mut state, 0, 0);

    assert_eq!(actions, vec![tuke::Action::Redraw]);
}

#[test]
fn resize_to_the_same_size_asks_for_nothing() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = state.update(tuke::Event::Resize { size: test_size() });

    assert!(actions.is_empty());
}

#[test]
fn resize_reports_the_new_grid_and_redraws() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = state.update(tuke::Event::Resize {
        size: tuinix::Size { rows: 50, cols: 40 },
    });

    // 50 rows minus the 9-row keyboard leaves a 41-row grid; the 3-column
    // layout still centres in 40 columns.
    assert_eq!(
        actions,
        vec![
            tuke::Action::ResizeSession(tuinix::Size { rows: 41, cols: 40 }),
            tuke::Action::Redraw,
        ]
    );
    assert_eq!(state.grid_size().rows, 41);
}

#[test]
fn pointer_release_outside_every_key_asks_for_nothing() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // Row 15 is above the keyboard (which starts at row 31), so it hits no key.
    let actions = state.update(tuke::Event::PointerRelease {
        position: tuinix::Position { row: 15, col: 13 },
    });

    assert!(actions.is_empty());
}

#[test]
fn modifier_key_only_redraws() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = press(&mut state, 0, 0);

    assert_eq!(actions, vec![tuke::Action::Redraw]);
    assert_eq!(state.keys()[0].press, tuke::KeyPressState::OneshotActivated);
}

#[test]
fn normal_key_sends_its_character() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = press(&mut state, 3, 0);

    let key = sent_key(&actions).expect("a normal key sends exactly one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
    assert_eq!(key.modifiers, termnix::Modifiers::new());
}

#[test]
fn oneshot_ctrl_applies_to_the_next_key_only() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

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
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

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
fn oneshot_ctrl_is_not_swallowed_by_a_key_that_ignores_it() {
    let mut layout = test_layout();
    layout.keys.push(key(tuke::KeyCode::Enter, 9, 0));
    layout.keys.push(key(tuke::KeyCode::Char('d'), 12, 0));
    let mut state = tuke::State::new(layout_set(layout), test_size(), None);

    // Tap Ctrl, then Enter: Enter cannot carry Ctrl, so it is sent plain and
    // the one-shot Ctrl is used up here rather than leaking to the next key.
    press(&mut state, 0, 0);
    let enter = press(&mut state, 9, 0);
    let next = press(&mut state, 12, 0);

    let enter_key = sent_key(&enter).expect("Enter sends one key event");
    assert_eq!(enter_key.code, termnix::KeyCode::Enter);
    assert_eq!(enter_key.modifiers, termnix::Modifiers::new());

    let next_key = sent_key(&next).expect("the key after Enter sends one key event");
    assert_eq!(next_key.code, termnix::KeyCode::Char('d'));
    assert_eq!(
        next_key.modifiers,
        termnix::Modifiers::new(),
        "Ctrl leaked past the key it was armed for"
    );
}

#[test]
fn oneshot_ctrl_arms_the_next_key_only_after_ignored_keys() {
    let mut layout = test_layout();
    layout.keys.push(key(tuke::KeyCode::Tab, 9, 0));
    layout.keys.push(key(tuke::KeyCode::Backspace, 12, 0));
    let mut state = tuke::State::new(layout_set(layout), test_size(), None);

    // Even after two keys that cannot carry Ctrl, no Ctrl is delivered later:
    // each one-shot is consumed by the key it was armed for.
    press(&mut state, 0, 0);
    press(&mut state, 9, 0);
    let after_tab = press(&mut state, 3, 0);
    let key = sent_key(&after_tab).expect("a normal key still sends one key event");
    assert_eq!(key.modifiers, termnix::Modifiers::new());
}

#[test]
fn shift_selects_the_shift_label() {
    let mut layout = test_layout();
    layout.keys.push(key(tuke::KeyCode::Shift, 9, 0));
    let mut state = tuke::State::new(layout_set(layout), test_size(), None);

    press(&mut state, 9, 0);
    assert!(state.is_shift_active());

    let actions = press(&mut state, 3, 0);
    let key = sent_key(&actions).expect("Shift + b sends one key event");
    assert_eq!(key.code, termnix::KeyCode::Char('B'));
}

/// The host mouse event a test sends, with no modifiers unless given.
fn host_mouse(kind: tuinix::MouseInputKind, row: usize, col: usize) -> tuke::Event {
    tuke::Event::Mouse {
        kind,
        position: tuinix::Position { row, col },
        ctrl: false,
        alt: false,
        shift: false,
    }
}

/// The mouse event an action list sends, if it sends exactly one.
fn sent_mouse(actions: &[tuke::Action]) -> Option<&termnix::MouseEvent> {
    match actions {
        [tuke::Action::SendMouse(mouse)] => Some(mouse),
        _ => None,
    }
}

#[test]
fn a_mouse_press_in_the_grid_goes_to_the_child() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = state.update(host_mouse(tuinix::MouseInputKind::LeftPress, 4, 9));

    let mouse = sent_mouse(&actions).expect("a click in the grid is forwarded");
    assert_eq!(
        mouse.kind,
        termnix::MouseEventKind::Press(termnix::MouseButton::Left)
    );
    // Row 4, column 9 is already a grid position: the grid starts at the
    // screen's top-left corner, so only the keyboard is indented.
    assert_eq!(mouse.position, termnix::Position { row: 4, col: 9 });
}

#[test]
fn a_mouse_release_outside_the_keyboard_goes_to_the_child() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // The keyboard starts at row 31, so row 4 is in the grid: the release
    // pairs with the press above rather than pressing a soft key.
    let actions = state.update(host_mouse(tuinix::MouseInputKind::LeftRelease, 4, 9));

    let mouse = sent_mouse(&actions).expect("a release in the grid is forwarded");
    assert_eq!(
        mouse.kind,
        termnix::MouseEventKind::Release(termnix::MouseButton::Left)
    );
}

#[test]
fn a_click_on_a_key_never_reaches_the_child() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);
    let centre = screen_centre(&state, 3, 0);

    // The whole gesture is the keyboard's: the child's reporting would
    // otherwise see a press it never painted a target for.
    for kind in [
        tuinix::MouseInputKind::LeftPress,
        tuinix::MouseInputKind::LeftRelease,
        tuinix::MouseInputKind::Drag,
        tuinix::MouseInputKind::ScrollUp,
    ] {
        let actions = state.update(host_mouse(kind, centre.row, centre.col));
        assert!(
            sent_mouse(&actions).is_none(),
            "{kind:?} on a key reached the child: {actions:?}"
        );
    }
}

#[test]
fn a_left_release_on_a_key_presses_it() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);
    let centre = screen_centre(&state, 3, 0);

    let actions = state.update(host_mouse(
        tuinix::MouseInputKind::LeftRelease,
        centre.row,
        centre.col,
    ));

    let key = sent_key(&actions).expect("releasing on a key presses it");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
}

#[test]
fn a_drag_reports_the_button_held_by_the_preceding_press() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // The host's drag does not name the button, so the core has to remember
    // the press: a guest told "moved" with no button cannot draw a selection.
    state.update(host_mouse(tuinix::MouseInputKind::RightPress, 4, 9));
    let actions = state.update(host_mouse(tuinix::MouseInputKind::Drag, 5, 10));

    let mouse = sent_mouse(&actions).expect("a drag in the grid is forwarded");
    assert_eq!(
        mouse.kind,
        termnix::MouseEventKind::Motion {
            button: Some(termnix::MouseButton::Right)
        }
    );
    assert_eq!(mouse.position, termnix::Position { row: 5, col: 10 });
}

#[test]
fn a_drag_with_no_button_held_is_a_bare_move() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // A move the host reports without a press is not a drag: guessing a button
    // would make the child select text the user never selected.
    let actions = state.update(host_mouse(tuinix::MouseInputKind::Drag, 5, 10));

    let mouse = sent_mouse(&actions).expect("a bare move in the grid is forwarded");
    assert_eq!(mouse.kind, termnix::MouseEventKind::Motion { button: None });
}

#[test]
fn the_held_button_is_forgotten_at_release() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    state.update(host_mouse(tuinix::MouseInputKind::LeftPress, 4, 9));
    state.update(host_mouse(tuinix::MouseInputKind::LeftRelease, 4, 9));
    let actions = state.update(host_mouse(tuinix::MouseInputKind::Drag, 6, 11));

    let mouse = sent_mouse(&actions).expect("a move after release is forwarded");
    assert_eq!(
        mouse.kind,
        termnix::MouseEventKind::Motion { button: None },
        "the button was still held after its release"
    );
}

#[test]
fn the_wheel_is_sent_as_a_press_of_a_wheel_button() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let up = state.update(host_mouse(tuinix::MouseInputKind::ScrollUp, 4, 9));
    let down = state.update(host_mouse(tuinix::MouseInputKind::ScrollDown, 4, 9));

    let up = sent_mouse(&up).expect("the wheel up is forwarded");
    assert_eq!(
        up.kind,
        termnix::MouseEventKind::Press(termnix::MouseButton::WheelUp)
    );
    let down = sent_mouse(&down).expect("the wheel down is forwarded");
    assert_eq!(
        down.kind,
        termnix::MouseEventKind::Press(termnix::MouseButton::WheelDown)
    );
}

#[test]
fn a_mouse_event_below_the_grid_is_dropped() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // Row 35 is below the 31-row grid but above nothing on screen: the child
    // never painted it, so sending it would point at a cell that is not there
    // rather than at the nearest one.
    let actions = state.update(host_mouse(tuinix::MouseInputKind::LeftPress, 35, 9));

    assert!(actions.is_empty(), "expected no actions, got {actions:?}");
}

#[test]
fn a_floating_keyboard_sends_a_drag_over_a_key_to_the_child() {
    // With the keyboard floating, the grid is the whole terminal, so a
    // position over a key is still a grid position: a drag there is the
    // child's, only the left release presses the key.
    let size = tuinix::Size { rows: 40, cols: 30 };
    let anchor = tuke::KeyboardPos { col: 13, rows: 0 };
    let mut state = tuke::State::new(layout_set(test_layout()), size, Some(anchor));
    let centre = screen_centre(&state, 3, 0);

    let actions = state.update(host_mouse(
        tuinix::MouseInputKind::Drag,
        centre.row,
        centre.col,
    ));

    let mouse = sent_mouse(&actions).expect("a drag over a key is forwarded");
    assert_eq!(mouse.kind, termnix::MouseEventKind::Motion { button: None });
}

#[test]
fn a_floating_keyboard_still_presses_a_key_on_left_release() {
    let size = tuinix::Size { rows: 40, cols: 30 };
    let anchor = tuke::KeyboardPos { col: 13, rows: 0 };
    let mut state = tuke::State::new(layout_set(test_layout()), size, Some(anchor));
    let centre = screen_centre(&state, 3, 0);

    let actions = state.update(host_mouse(
        tuinix::MouseInputKind::LeftRelease,
        centre.row,
        centre.col,
    ));

    let key = sent_key(&actions).expect("releasing on a key presses it");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
}

#[test]
fn a_floating_keyboard_follows_a_resize() {
    // The position is pinned to the terminal's bottom-left corner, so when the
    // terminal grows the keyboard moves with the corner rather than staying
    // where it was.
    let anchor = tuke::KeyboardPos { col: 13, rows: 0 };
    let mut state = tuke::State::new(
        layout_set(test_layout()),
        tuinix::Size { rows: 40, cols: 30 },
        Some(anchor),
    );
    assert_eq!(state.offset().row, 31);

    state.update(tuke::Event::Resize {
        size: tuinix::Size { rows: 50, cols: 30 },
    });

    // Ten more rows: the keyboard's last row is now 49, so it starts at 41.
    assert_eq!(state.offset().row, 41);
    assert_eq!(state.grid_size(), tuinix::Size { rows: 50, cols: 30 });
}

#[test]
fn a_floating_keyboard_keeps_its_distance_from_the_bottom() {
    // With two rows of clearance, growing the terminal keeps two rows of
    // clearance from the new bottom instead of counting from the old one.
    let anchor = tuke::KeyboardPos { col: 0, rows: 2 };
    let mut state = tuke::State::new(
        layout_set(test_layout()),
        tuinix::Size { rows: 40, cols: 30 },
        Some(anchor),
    );
    // Last row 37 (40 - 1 - 2), nine rows tall, so it starts at 29.
    assert_eq!(state.offset().row, 29);

    state.update(tuke::Event::Resize {
        size: tuinix::Size { rows: 50, cols: 30 },
    });

    // Last row 47 (50 - 1 - 2), so it starts at 39.
    assert_eq!(state.offset().row, 39);
}

#[test]
fn a_floating_keyboard_keeps_the_grid_at_full_size() {
    let size = tuinix::Size { rows: 40, cols: 30 };
    let anchor = tuke::KeyboardPos { col: 13, rows: 0 };
    let state = tuke::State::new(layout_set(test_layout()), size, Some(anchor));

    assert!(state.is_overlay());
    assert_eq!(state.grid_size(), size);
    // The keyboard's last row is the anchor row, so a nine-row keyboard ends
    // at row 39 and starts at row 31.
    assert_eq!(state.offset().row, 31);
    assert_eq!(state.offset().col, 13);
}

#[test]
fn a_docked_keyboard_swallows_a_key_press_that_is_not_a_release() {
    // The docked keyboard is not part of the grid, so the child never painted
    // the key's position: a press there must not leak to it.
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);
    let centre = screen_centre(&state, 3, 0);

    let actions = state.update(host_mouse(
        tuinix::MouseInputKind::LeftPress,
        centre.row,
        centre.col,
    ));

    assert!(actions.is_empty(), "expected no actions, got {actions:?}");
}

#[test]
fn a_mouse_event_beside_the_keyboard_is_dropped() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // Column 29 is right of the 3-column keyboard's keys but inside the 30
    // column grid, so this is a grid click at the far right edge.
    let actions = state.update(host_mouse(tuinix::MouseInputKind::LeftPress, 20, 29));

    let mouse = sent_mouse(&actions).expect("the grid spans the full width");
    assert_eq!(mouse.position, termnix::Position { row: 20, col: 29 });
}

#[test]
fn mouse_modifiers_are_copied_through() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = state.update(tuke::Event::Mouse {
        kind: tuinix::MouseInputKind::LeftPress,
        position: tuinix::Position { row: 4, col: 9 },
        ctrl: true,
        alt: true,
        shift: true,
    });

    let mouse = sent_mouse(&actions).expect("a modified click in the grid is forwarded");
    assert_eq!(
        mouse.modifiers,
        termnix::Modifiers {
            ctrl: true,
            alt: true,
            shift: true,
        }
    );
}

#[test]
fn the_grid_and_keyboard_split_every_mouse_position() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let reached_grid = Cell::new(0usize);
    let reached_keyboard = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let rows = noprop::sample_usize_in(ctx, 9..=60);
        let cols = noprop::sample_usize_in(ctx, 3..=80);
        let size = tuinix::Size { rows, cols };
        let mut state = tuke::State::new(layout_set(test_layout()), size, None);
        let row = noprop::sample_usize_in(ctx, 0..rows);
        let col = noprop::sample_usize_in(ctx, 0..cols);

        // A press is the clearest probe: it is forwarded inside the grid and
        // swallowed anywhere tuke owns a key, and never both.
        let actions = state.update(host_mouse(tuinix::MouseInputKind::LeftPress, row, col));
        let forwarded = sent_mouse(&actions).is_some();
        let inside_grid = row < state.grid_size().rows;
        assert_eq!(
            forwarded, inside_grid,
            "({row}, {col}) on a {rows}x{cols} screen: forwarded={forwarded}"
        );
        if forwarded {
            reached_grid.set(reached_grid.get() + 1);
        } else {
            reached_keyboard.set(reached_keyboard.get() + 1);
        }
        Ok(())
    })?;

    // Both branches have to be exercised: a generator that only produced grid
    // positions would never test that a click on a key is swallowed.
    assert!(
        reached_grid.get() > 0,
        "no case landed in the grid\n{runner}"
    );
    assert!(
        reached_keyboard.get() > 0,
        "no case landed on a key\n{runner}"
    );
    Ok(())
}

/// The host key event a test sends, with no modifiers unless given.
fn host_key(code: tuinix::KeyCode, ctrl: bool, alt: bool) -> tuke::Event {
    tuke::Event::Key { code, ctrl, alt }
}

/// The paste payload an action list sends, if it sends exactly one.
fn sent_paste(actions: &[tuke::Action]) -> Option<&str> {
    match actions {
        [tuke::Action::SendPaste(text)] => Some(text),
        _ => None,
    }
}

#[test]
fn every_host_key_is_forwarded_to_the_child() {
    // tuke reserves no key of its own, so there is no key it can swallow: even
    // `q` and `C-c` reach the child.
    let cases = [
        (tuinix::KeyCode::Char('q'), false, false),
        (tuinix::KeyCode::Char('c'), true, false),
        (tuinix::KeyCode::Char('a'), false, false),
        (tuinix::KeyCode::Enter, false, false),
        (tuinix::KeyCode::Escape, false, true),
        (tuinix::KeyCode::F(5), false, false),
    ];
    for (code, ctrl, alt) in cases {
        let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

        let actions = state.update(host_key(code, ctrl, alt));

        assert_eq!(actions.len(), 1, "one key event for {code:?}");
        let tuke::Action::SendKey(key) = &actions[0] else {
            panic!("expected a SendKey for {code:?}, got {actions:?}");
        };
        assert_eq!(key.modifiers.ctrl, ctrl, "ctrl for {code:?}");
        assert_eq!(key.modifiers.alt, alt, "alt for {code:?}");
    }
}

#[test]
fn host_key_codes_map_onto_the_guest_ones() {
    // A spurious modifier would change the byte the child receives, so the
    // mapping is checked key by key rather than only for the ones with names.
    let cases = [
        (tuinix::KeyCode::Char('x'), termnix::KeyCode::Char('x')),
        (tuinix::KeyCode::Enter, termnix::KeyCode::Enter),
        (tuinix::KeyCode::Escape, termnix::KeyCode::Escape),
        (tuinix::KeyCode::Backspace, termnix::KeyCode::Backspace),
        (tuinix::KeyCode::Tab, termnix::KeyCode::Tab),
        (tuinix::KeyCode::Delete, termnix::KeyCode::Delete),
        (tuinix::KeyCode::Insert, termnix::KeyCode::Insert),
        (tuinix::KeyCode::Up, termnix::KeyCode::Up),
        (tuinix::KeyCode::Down, termnix::KeyCode::Down),
        (tuinix::KeyCode::Left, termnix::KeyCode::Left),
        (tuinix::KeyCode::Right, termnix::KeyCode::Right),
        (tuinix::KeyCode::Home, termnix::KeyCode::Home),
        (tuinix::KeyCode::End, termnix::KeyCode::End),
        (tuinix::KeyCode::PageUp, termnix::KeyCode::PageUp),
        (tuinix::KeyCode::PageDown, termnix::KeyCode::PageDown),
        (tuinix::KeyCode::F(3), termnix::KeyCode::Function(3)),
    ];
    for (host, guest) in cases {
        let mapped = host_key(host, false, false)
            .to_guest_key()
            .unwrap_or_else(|| panic!("no mapping for {host:?}"));
        assert_eq!(mapped.code, guest, "code for {host:?}");
        assert!(!mapped.modifiers.shift, "shift for {host:?}");
    }
}

#[test]
fn back_tab_becomes_tab_with_shift() {
    let mapped = host_key(tuinix::KeyCode::BackTab, false, false)
        .to_guest_key()
        .expect("BackTab maps");

    assert_eq!(mapped.code, termnix::KeyCode::Tab);
    assert!(mapped.modifiers.shift);
}

#[test]
fn a_paste_is_forwarded_as_one_paste() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    let actions = state.update(tuke::Event::Paste {
        bytes: b"hello\nworld".to_vec(),
    });

    // One paste, not the twelve key presses it spells: a newline in a paste is
    // text, and the child is told so when it is handed over whole.
    assert_eq!(sent_paste(&actions), Some("hello\nworld"));
}

#[test]
fn an_empty_paste_is_still_forwarded() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // The host terminal saw the markers, so the child is told a paste happened
    // even though it carried nothing: that can be meaningful to an editor.
    let actions = state.update(tuke::Event::Paste { bytes: Vec::new() });

    assert_eq!(sent_paste(&actions), Some(""));
}

#[test]
fn a_paste_leaves_the_keyboard_state_alone() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // A paste does not go through a soft key, so it neither consumes a one-shot
    // modifier nor changes what any key looks like.
    press(&mut state, 0, 0);
    state.update(tuke::Event::Paste {
        bytes: b"hi".to_vec(),
    });
    let actions = press(&mut state, 3, 0);

    let key = sent_key(&actions).expect("the key after a paste is still sent");
    assert_eq!(key.code, termnix::KeyCode::Char('b'));
    assert!(
        key.modifiers.ctrl,
        "the one-shot Ctrl armed before the paste should still apply"
    );
}

#[test]
fn a_paste_that_is_not_utf8_is_dropped() {
    let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

    // A guest paste is a string, so bytes that are not text cannot be sent as
    // one. Nothing is sent rather than fabricating keys for them: the child
    // must not receive bytes the user never typed.
    let actions = state.update(tuke::Event::Paste {
        bytes: vec![0xff, 0xfe],
    });

    assert!(actions.is_empty(), "expected no actions, got {actions:?}");
}

#[test]
fn every_paste_round_trips_through_the_transition() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time("TUKE_SEED")?;
    let reached_utf8 = Cell::new(0usize);
    let reached_non_utf8 = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);
    runner.run(256, |ctx| {
        let len = noprop::sample_usize_in(ctx, 0..=32);
        let bytes = noprop::sample_bytes_vec(ctx, len);
        let mut state = tuke::State::new(layout_set(test_layout()), test_size(), None);

        let actions = state.update(tuke::Event::Paste {
            bytes: bytes.clone(),
        });

        match std::str::from_utf8(&bytes) {
            Ok(text) => {
                assert_eq!(
                    sent_paste(&actions),
                    Some(text),
                    "UTF-8 paste {bytes:?} should be forwarded unchanged"
                );
                reached_utf8.set(reached_utf8.get() + 1);
            }
            Err(_) => {
                assert!(
                    actions.is_empty(),
                    "non-UTF-8 paste {bytes:?} should be dropped, got {actions:?}"
                );
                reached_non_utf8.set(reached_non_utf8.get() + 1);
            }
        }
        Ok(())
    })?;

    // Both branches have to be exercised for the property to mean anything:
    // a generator that only produced text would never test the drop.
    assert!(
        reached_utf8.get() > 0,
        "no case pasted valid UTF-8\n{runner}"
    );
    assert!(
        reached_non_utf8.get() > 0,
        "no case pasted invalid UTF-8\n{runner}"
    );
    Ok(())
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
        let mut state = tuke::State::new(layout_set(test_layout()), size, None);

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
            let actions = state.update(tuke::Event::PointerRelease { position: centre });
            if index == 0 {
                // Ctrl is a modifier, so its press only redraws.
                assert_eq!(actions, vec![tuke::Action::Redraw]);
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
