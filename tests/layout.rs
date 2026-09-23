//! Tests for the layout model: JSONC parsing and key code mapping.

/// Writes `text` to a fresh temporary file and returns its path.
///
/// Each call gets its own name, so the tests can run in parallel without
/// sharing a file.
fn write_temp(text: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "tuke-layout-test-{}-{}.jsonc",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).expect("write layout file");
    path
}

/// Parses a single JSON string literal as a `KeyCode`.
///
/// The layout file writes a key code as a quoted string, so the test parses
/// `"<code>"` the same way the layout loader does.
fn parse_code(literal: &str) -> Result<tuke::KeyCode, nojson::JsonParseError> {
    let (json, _) = nojson::RawJson::parse_jsonc(literal).expect("literal is valid JSONC");
    tuke::KeyCode::try_from(json.value())
}

/// The code a send key sends when Shift is not active.
///
/// A key sends a code, switches layouts, or types a shortcut; these tests only
/// build send keys here, so this unwraps the code rather than threading a
/// `Result` through every assertion.
fn code_of(key: &tuke::Key) -> tuke::KeyCode {
    match key.action {
        tuke::KeyAction::Send { code, .. } => code,
        tuke::KeyAction::Switch { .. } => panic!("expected a send key, got a switch key"),
        tuke::KeyAction::Shortcut { .. } => panic!("expected a send key, got a shortcut key"),
    }
}

#[test]
fn key_codes_round_trip_through_their_textual_form() {
    let codes = [
        tuke::KeyCode::Char('a'),
        tuke::KeyCode::Char('7'),
        tuke::KeyCode::Char('!'),
        tuke::KeyCode::Shift,
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Alt,
        tuke::KeyCode::Up,
        tuke::KeyCode::Down,
        tuke::KeyCode::Left,
        tuke::KeyCode::Right,
        tuke::KeyCode::Enter,
        tuke::KeyCode::Backspace,
        tuke::KeyCode::Delete,
        tuke::KeyCode::Escape,
        tuke::KeyCode::Tab,
        tuke::KeyCode::BackTab,
    ];

    for code in codes {
        let text = code.to_string();
        let literal = format!("\"{text}\"");
        let parsed =
            parse_code(&literal).unwrap_or_else(|e| panic!("failed to parse {literal}: {e}"));
        assert_eq!(parsed, code, "round trip changed {code:?}");
    }
}

#[test]
fn bspace_and_btab_use_the_tmux_spelling() {
    assert_eq!(tuke::KeyCode::Backspace.to_string(), "BSpace");
    assert_eq!(tuke::KeyCode::BackTab.to_string(), "BTab");
}

#[test]
fn unknown_key_code_is_rejected() {
    let result = parse_code("\"F1\"");
    assert!(result.is_err(), "F1 is not a tuke key code");
}

#[test]
fn multi_character_string_is_rejected() {
    // The parser accepts a single character string; "ab" is not one.
    let result = parse_code("\"ab\"");
    assert!(result.is_err());
}

#[test]
fn modifiers_map_to_no_termnix_key() {
    for code in [
        tuke::KeyCode::Shift,
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Alt,
    ] {
        assert!(code.is_modifier());
        assert_eq!(code.to_termnix(), None, "{code:?} should not be sent alone");
    }
}

#[test]
fn named_keys_map_to_the_matching_termnix_code() {
    let cases = [
        (tuke::KeyCode::Up, termnix::KeyCode::Up),
        (tuke::KeyCode::Down, termnix::KeyCode::Down),
        (tuke::KeyCode::Left, termnix::KeyCode::Left),
        (tuke::KeyCode::Right, termnix::KeyCode::Right),
        (tuke::KeyCode::Enter, termnix::KeyCode::Enter),
        (tuke::KeyCode::Backspace, termnix::KeyCode::Backspace),
        (tuke::KeyCode::Delete, termnix::KeyCode::Delete),
        (tuke::KeyCode::Escape, termnix::KeyCode::Escape),
        (tuke::KeyCode::Tab, termnix::KeyCode::Tab),
        (tuke::KeyCode::Char('x'), termnix::KeyCode::Char('x')),
    ];

    for (code, expected) in cases {
        assert_eq!(code.to_termnix(), Some(expected), "for {code:?}");
    }
}

#[test]
fn back_tab_maps_to_tab() {
    // Shift is carried in the modifiers, so BTab maps to the Tab key code.
    assert_eq!(
        tuke::KeyCode::BackTab.to_termnix(),
        Some(termnix::KeyCode::Tab)
    );
}

#[test]
fn default_shift_code_uppercases_characters() {
    assert_eq!(
        tuke::KeyCode::Char('a').default_shift_code(),
        tuke::KeyCode::Char('A')
    );
    assert_eq!(
        tuke::KeyCode::Tab.default_shift_code(),
        tuke::KeyCode::BackTab
    );
    assert_eq!(
        tuke::KeyCode::Enter.default_shift_code(),
        tuke::KeyCode::Enter
    );
}

#[test]
fn default_layout_loads_and_declares_its_keys() {
    let layout = tuke::Layout::default();

    // The default layout is the board shown at startup: the first one the file
    // declares. It carries letters and a space, which is the least a shell
    // needs before any switch key is pressed.
    assert!(
        layout
            .keys
            .iter()
            .filter_map(|k| match k.action {
                tuke::KeyAction::Send { code, .. } => Some(code),
                tuke::KeyAction::Switch { .. } | tuke::KeyAction::Shortcut { .. } => None,
            })
            .any(|code| code == tuke::KeyCode::Char('j')),
        "the startup board carries the letters"
    );
    assert!(
        layout
            .keys
            .iter()
            .any(|k| code_of(k) == tuke::KeyCode::Char(' ')),
        "the startup board carries a space"
    );
}

#[test]
fn layout_from_a_file_matches_the_text() {
    let path = write_temp(r#"[{"key": "a", "size": {"width": 5, "height": 5}}]"#);

    let layout = tuke::Layout::load_from_file(&path).expect("parse layout file");
    let _ = std::fs::remove_file(&path);

    assert_eq!(layout.keys.len(), 1);
    assert_eq!(code_of(&layout.keys[0]), tuke::KeyCode::Char('a'));
    assert_eq!(
        layout.keys[0].region.size,
        tuinix::Size { rows: 5, cols: 5 }
    );
    assert!(layout.preview.is_none());
}

#[test]
fn a_switch_key_parses_its_target() {
    // A switch key spells its action as `{"switch_to": NAME}` under `key`,
    // the same member a send key uses, so one key form covers both. The name
    // has to exist for the file to load, so this declares the target too.
    let set = parse_set(
        r#"[
            {"layout": "main"},
            {"key": {"switch_to": "minimal"}, "size": {"width": 5, "height": 5}},
            {"layout": "minimal"},
            {"key": "a"}
        ]"#,
    );
    let layout = set.get("main").expect("main layout");

    assert_eq!(layout.keys.len(), 1);
    assert_eq!(
        layout.keys[0].action,
        tuke::KeyAction::Switch {
            to: "minimal".to_string()
        }
    );
}

#[test]
fn a_switch_key_ignores_the_shift_member() {
    // A switch key has no code to shift, so a `shift` beside it is not an
    // error but the switch is still the whole action.
    let set = parse_set(
        r#"[
            {"layout": "main"},
            {"key": {"switch_to": "other"}, "shift": "a"},
            {"layout": "other"},
            {"key": "b"}
        ]"#,
    );

    assert_eq!(
        set.get("main").expect("main layout").keys[0].action,
        tuke::KeyAction::Switch {
            to: "other".to_string()
        }
    );
}

#[test]
fn a_switch_to_a_missing_layout_is_rejected() {
    // The target is resolved when the file is read, not when the key is
    // pressed, so a switch that names a layout the file does not define fails
    // to load: an inert key the user cannot see is a typo, not a feature.
    let result = tuke::LayoutSet::load_from_file(write_temp(
        r#"[{"layout": "only"}, {"key": {"switch_to": "nowhere"}}]"#,
    ));

    assert!(
        result.is_err(),
        "a switch to a layout the file never declares is a load error"
    );
}

#[test]
fn a_switch_may_name_a_layout_declared_later() {
    // The entries are read in order, so a switch can point forward: the whole
    // file is read before its references are checked.
    let set = parse_set(
        r#"[
            {"layout": "first"},
            {"key": {"switch_to": "second"}},
            {"layout": "second"},
            {"key": {"switch_to": "first"}}
        ]"#,
    );

    assert_eq!(
        set.get("first").expect("first").keys[0].action,
        tuke::KeyAction::Switch {
            to: "second".to_string()
        }
    );
    assert_eq!(
        set.get("second").expect("second").keys[0].action,
        tuke::KeyAction::Switch {
            to: "first".to_string()
        }
    );
}

#[test]
fn a_switch_in_an_unnamed_file_may_name_default() {
    // A file with no `{"layout": …}` entry is one layout called `default`,
    // and that implicit name is a target like any other.
    let set = parse_set(r#"[{"key": {"switch_to": "default"}}]"#);

    assert_eq!(set.layouts().len(), 1);
    assert_eq!(
        set.first().keys[0].action,
        tuke::KeyAction::Switch {
            to: "default".to_string()
        }
    );
}

#[test]
fn a_switch_without_a_target_is_rejected() {
    let result = tuke::Layout::load_from_file(write_temp(r#"[{"key": {"other": 1}}]"#));

    assert!(result.is_err(), "a switch object needs a switch_to member");
}

#[test]
fn a_shortcut_key_carries_its_label_and_text() {
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"key": {"shortcut": {"label": "tell", "text": "attini tell"}}}]"#,
    ))
    .expect("a shortcut key loads");

    assert_eq!(
        layout.keys[0].action,
        tuke::KeyAction::Shortcut {
            label: "tell".to_string(),
            text: "attini tell".to_string(),
        }
    );
}

#[test]
fn a_shortcut_keeps_its_text_verbatim() {
    // The text is a command line the user typed, so tuke carries it as
    // written: the spaces inside it are part of it, and a label is not a
    // prefix of it that could stand in.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"key": {"shortcut": {"label": "ap", "text": "attini  approve --now"}}}]"#,
    ))
    .expect("a shortcut key loads");

    assert_eq!(
        layout.keys[0].action,
        tuke::KeyAction::Shortcut {
            label: "ap".to_string(),
            text: "attini  approve --now".to_string(),
        }
    );
}

#[test]
fn a_shortcut_without_a_label_is_rejected() {
    // The label is what the key draws, and the text is usually too long to
    // draw, so a shortcut that names no label has nothing to show.
    let result = tuke::Layout::load_from_file(write_temp(
        r#"[{"key": {"shortcut": {"text": "attini tell"}}}]"#,
    ));

    assert!(result.is_err(), "a shortcut needs a label");
}

#[test]
fn a_shortcut_without_a_text_is_rejected() {
    let result =
        tuke::Layout::load_from_file(write_temp(r#"[{"key": {"shortcut": {"label": "tell"}}}]"#));

    assert!(result.is_err(), "a shortcut needs a text");
}

#[test]
fn a_shortcut_with_a_shift_code_is_rejected() {
    // A shortcut types a string, which has no shifted form to choose between,
    // so a `shift` beside it names something that cannot happen. It is
    // reported rather than ignored, so a layout cannot claim a behaviour it
    // does not have.
    let result = tuke::Layout::load_from_file(write_temp(
        r#"[{"key": {"shortcut": {"label": "tell", "text": "attini tell"}}, "shift": "a"}]"#,
    ));

    assert!(result.is_err(), "a shortcut cannot carry a shift code");
}

#[test]
fn a_shortcut_keeps_its_size() {
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"key": {"shortcut": {"label": "tell", "text": "attini tell"}},
            "size": {"width": 7, "height": 3}}]"#,
    ))
    .expect("a shortcut key loads");

    assert_eq!(layout.keys[0].region.size.cols, 7);
    assert_eq!(layout.keys[0].region.size.rows, 3);
}

#[test]
fn keys_are_a_padding_apart_by_default() {
    // One column is the long-standing gap, so a layout that says nothing
    // keeps the spacing it always had.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"key": "a"},
            {"key": "b"}]"#,
    ))
    .expect("two keys load");

    assert_eq!(layout.keys[0].padding, 1);
    assert_eq!(layout.keys[1].region.position.col, 4);
}

#[test]
fn a_padding_of_zero_puts_the_next_key_flush() {
    // The point of the member: the gap is not part of the key, so a layout
    // that wants the columns can have them.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"key": "a", "padding": 0},
            {"key": "b"}]"#,
    ))
    .expect("two keys load");

    assert_eq!(layout.keys[0].padding, 0);
    assert_eq!(layout.keys[1].region.position.col, 3);
}

#[test]
fn the_padding_member_can_be_set_for_the_whole_layout() {
    // A compact board writes one entry rather than one per key.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"default_padding": 0},
            {"key": "a"},
            {"key": "b"}]"#,
    ))
    .expect("two keys load");

    assert_eq!(layout.keys[1].region.position.col, 3);
}

#[test]
fn a_padding_on_one_key_overrides_the_layout_default() {
    // The two members are independent: the default only fills in the keys
    // that say nothing, and a key may widen a row the layout made compact.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"default_padding": 0},
            {"key": "a"},
            {"key": "b", "padding": 2},
            {"key": "c"}]"#,
    ))
    .expect("three keys load");

    assert_eq!(layout.keys[1].region.position.col, 3);
    assert_eq!(layout.keys[2].region.position.col, 8);
}

#[test]
fn a_padding_wider_than_the_screen_is_read_as_written() {
    // The loader does not know the terminal, so it records the gap and lets
    // the keyboard be drawn past the edge and cropped, as a wide layout is.
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"key": "a", "padding": 100},
            {"key": "b"}]"#,
    ))
    .expect("two keys load");

    assert_eq!(layout.keys[1].region.position.col, 103);
}

#[test]
fn a_negative_padding_is_rejected() {
    let result = tuke::Layout::load_from_file(write_temp(r#"[{"key": "a", "padding": -1}]"#));

    assert!(result.is_err(), "a padding below zero is not a count");
}

#[test]
fn a_padding_on_a_switch_key_is_honoured() {
    // Padding belongs to the key, not to the kind of thing it does.
    let set = parse_set(
        r#"[{"default_size": {"width": 3, "height": 3}},
            {"key": {"switch_to": "default"}, "padding": 4},
            {"key": "a"}]"#,
    );
    let layout = &set.layouts()[0].layout;

    assert_eq!(layout.keys[0].padding, 4);
    assert_eq!(layout.keys[1].region.position.col, 7);
}

/// The rightmost column any key or the preview extends to, in layout cells.
fn layout_cols(layout: &tuke::Layout) -> usize {
    layout
        .keys
        .iter()
        .map(|k| k.region.position.col + k.region.size.cols)
        .chain(
            layout
                .preview
                .iter()
                .map(|p| p.region.position.col + p.region.size.cols),
        )
        .max()
        .unwrap_or_default()
}

/// Parses a layout set from an inline JSONC document.
fn parse_set(text: &str) -> tuke::LayoutSet {
    tuke::LayoutSet::load_from_file(write_temp(text))
        .unwrap_or_else(|e| panic!("parse {text}: {e}"))
}

#[test]
fn a_file_without_a_layout_entry_is_one_default_layout() {
    // The long-standing form: a bare array of keys. It must still load, as a
    // single layout named `default`.
    let set = parse_set(r#"[{"key": "a", "size": {"width": 5, "height": 5}}]"#);

    assert_eq!(set.layouts().len(), 1);
    assert_eq!(set.layouts()[0].name, "default");
    assert_eq!(set.layouts()[0].layout.keys.len(), 1);
    assert_eq!(
        code_of(&set.layouts()[0].layout.keys[0]),
        tuke::KeyCode::Char('a')
    );
}

#[test]
fn a_layout_entry_starts_a_named_layout() {
    let set = parse_set(
        r#"[
            {"layout": "first"},
            {"key": "a"},
            {"layout": "second"},
            {"key": "b"}
        ]"#,
    );

    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["first", "second"]);

    // Each layout restarts the cursor at the origin, so both keys land at
    // column 0 row 0 rather than continuing from the previous layout.
    let first = set.get("first").expect("first layout");
    let second = set.get("second").expect("second layout");
    assert_eq!(
        first.keys[0].region.position,
        tuinix::Position { row: 0, col: 0 }
    );
    assert_eq!(
        second.keys[0].region.position,
        tuinix::Position { row: 0, col: 0 }
    );
    assert_eq!(code_of(&first.keys[0]), tuke::KeyCode::Char('a'));
    assert_eq!(code_of(&second.keys[0]), tuke::KeyCode::Char('b'));
}

#[test]
fn entries_before_the_first_layout_entry_belong_to_default() {
    let set = parse_set(
        r#"[
            {"key": "a"},
            {"layout": "second"},
            {"key": "b"}
        ]"#,
    );

    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["default", "second"]);
    assert_eq!(set.get("default").expect("default").keys.len(), 1);
}

#[test]
fn the_first_layout_is_the_one_shown_at_startup() {
    let set = parse_set(
        r#"[
            {"layout": "shown"},
            {"key": "x"},
            {"layout": "hidden"},
            {"key": "y"}
        ]"#,
    );

    assert_eq!(code_of(&set.first().keys[0]), tuke::KeyCode::Char('x'));
}

#[test]
fn a_duplicate_layout_name_is_rejected() {
    let result = tuke::LayoutSet::load_from_file(write_temp(
        r#"[
            {"layout": "dup"},
            {"key": "a"},
            {"layout": "dup"},
            {"key": "b"}
        ]"#,
    ));

    assert!(result.is_err(), "the same layout name twice is ambiguous");
}

#[test]
fn an_unknown_layout_name_is_not_found() {
    let set = parse_set(r#"[{"layout": "only"}, {"key": "a"}]"#);

    assert!(set.get("missing").is_none());
    assert!(set.get("only").is_some());
}

#[test]
fn the_shipped_default_layout_declares_the_one_handed_boards() {
    // The default file names its boards, so `LayoutSet::default` has to read
    // them all rather than only the first. The names are what the switch keys
    // in the file refer to, so a renamed board would be a load error rather
    // than a layout the user could reach.
    let set: tuke::LayoutSet = tuke::LayoutSet::default();

    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MAIN", "SUB", "MIN"]);
    assert_eq!(set.first_name(), "MAIN", "the everyday board starts up");
}

/// Loads a layout that ships with tuke, by file name under `layouts/`.
///
/// A file with several layouts loads as its first one, which is the layout a
/// single-board file has and the one shown at startup.
fn shipped_layout(name: &str) -> tuke::Layout {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("layouts")
        .join(name);
    tuke::Layout::load_from_file(&path)
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()))
}

/// Loads every layout a shipped file defines, in declaration order.
fn shipped_layout_set(name: &str) -> tuke::LayoutSet {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("layouts")
        .join(name);
    tuke::LayoutSet::load_from_file(&path)
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()))
}

#[test]
fn the_default_layout_fits_in_eighty_columns() {
    let layout = shipped_layout("default.jsonc");

    // The point of the default layout is that it is usable on a narrow screen:
    // an 80-column terminal must be able to show it without cropping.
    assert!(
        layout_cols(&layout) <= 80,
        "the default is {} columns wide, over the 80-column budget",
        layout_cols(&layout)
    );
}

#[test]
fn the_default_layout_carries_the_keys_a_shell_needs() {
    // The file splits the keyboard over several boards, so the keys a shell
    // needs may live on one a switch key reaches rather than on the board
    // shown at startup. What this pins is that a hand can reach them all from
    // the keyboard as it ships, so it collects the codes from every board.
    let set = shipped_layout_set("default.jsonc");
    let everywhere: Vec<tuke::KeyCode> = set
        .layouts()
        .iter()
        .flat_map(|named| named.layout.keys.iter())
        .filter_map(|key| match key.action {
            tuke::KeyAction::Send { code, .. } => Some(code),
            tuke::KeyAction::Switch { .. } | tuke::KeyAction::Shortcut { .. } => None,
        })
        .collect();

    for c in 'a'..='z' {
        assert!(
            everywhere.contains(&tuke::KeyCode::Char(c)),
            "missing letter {c}"
        );
    }
    for c in '0'..='9' {
        assert!(
            everywhere.contains(&tuke::KeyCode::Char(c)),
            "missing digit {c}"
        );
    }
    for c in ['.', '-', '_', '/', '?', '"', '(', ')'] {
        assert!(
            everywhere.contains(&tuke::KeyCode::Char(c)),
            "missing symbol {c}"
        );
    }
    for code in [
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Escape,
        tuke::KeyCode::Tab,
        tuke::KeyCode::Enter,
        tuke::KeyCode::Backspace,
        tuke::KeyCode::Char(' '),
    ] {
        assert!(everywhere.contains(&code), "missing {code}");
    }
}

#[test]
fn the_default_layout_has_no_overlapping_keys() {
    let set = shipped_layout_set("default.jsonc");

    // Overlapping keys would make a hit test ambiguous, and the JSONC cursor
    // rules already place them, so a collision means the layout moved a key by
    // hand on top of another. Every board is checked: a hand-edit lands on
    // whichever one it was made in.
    for named in set.layouts() {
        let keys = &named.layout.keys;
        for (i, a) in keys.iter().enumerate() {
            for b in &keys[i + 1..] {
                let separated = a.region.position.col + a.region.size.cols <= b.region.position.col
                    || b.region.position.col + b.region.size.cols <= a.region.position.col
                    || a.region.position.row + a.region.size.rows <= b.region.position.row
                    || b.region.position.row + b.region.size.rows <= a.region.position.row;
                assert!(
                    separated,
                    "{}: keys {:?} at {:?} and {:?} at {:?} overlap",
                    named.name,
                    code_of(a),
                    a.region,
                    code_of(b),
                    b.region
                );
            }
        }
    }
}

#[test]
fn the_default_layout_labels_fit_their_keys() {
    let set = shipped_layout_set("default.jsonc");

    // The renderer crops a label that is too long for its key, so a cramped
    // label is not an error - but `BSpace` in a three-column key would read
    // `BSp`, which is a layout bug rather than a rendering one. A switch key
    // draws the layout it goes to rather than a code, so the text checked here
    // is the one the renderer derives from the key's own action.
    for named in set.layouts() {
        for key in &named.layout.keys {
            let label = match &key.action {
                tuke::KeyAction::Send { code, .. } => code.to_string(),
                tuke::KeyAction::Switch { to } => to.clone(),
                tuke::KeyAction::Shortcut { label, .. } => label.clone(),
            };
            let interior = key.region.size.cols.saturating_sub(2);
            assert!(
                label.chars().count() <= interior,
                "{}: label {label:?} needs {} columns but the key has {interior}",
                named.name,
                label.chars().count(),
            );
        }
    }
}

#[test]
fn a_size_below_the_minimum_is_rejected() {
    let path = write_temp(r#"[{"key": "a", "size": {"width": 2, "height": 5}}]"#);

    let result = tuke::Layout::load_from_file(&path);
    let _ = std::fs::remove_file(&path);

    assert!(result.is_err(), "a 2-column key is too small to draw");
}

#[test]
fn mini_left_switches_form_a_closed_set() {
    // The shipped one-handed board splits over three layouts: `MAIN`, the
    // board it starts on; `SUB`, a second board carrying the keys `MAIN` gives
    // up to stay compact; and `MIN`, a one-key board whose only job is to put
    // `MAIN` back. Every switch must resolve, or the file would not load (an
    // unknown target is rejected), so this pins the shape the file is meant to
    // have.
    let set = shipped_layout_set("default.jsonc");

    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MAIN", "SUB", "MIN"]);
    assert_eq!(set.first_name(), "MAIN", "the everyday board starts up");

    // Every switch in the file, as (where it is, where it goes).
    let switches: Vec<(String, String)> = set
        .layouts()
        .iter()
        .flat_map(|named| {
            named
                .layout
                .keys
                .iter()
                .filter_map(|key| match &key.action {
                    tuke::KeyAction::Switch { to } => Some((named.name.clone(), to.clone())),
                    tuke::KeyAction::Send { .. } | tuke::KeyAction::Shortcut { .. } => None,
                })
        })
        .collect();

    assert_eq!(
        switches,
        [
            ("MAIN".to_string(), "SUB".to_string()),
            ("MAIN".to_string(), "MIN".to_string()),
            ("SUB".to_string(), "MAIN".to_string()),
            ("SUB".to_string(), "MIN".to_string()),
            ("MIN".to_string(), "MAIN".to_string()),
        ],
        "MAIN and SUB both reach MIN, and MIN comes back to MAIN"
    );

    // Every layout the switches name is one the file defines, so no press can
    // land on a board that is not there.
    for (_, to) in &switches {
        assert!(set.get(to).is_some(), "switch targets unknown layout {to}");
    }

    // `MIN` is minimized: one key, and it is the switch back.
    let min = set.get("MIN").expect("MIN layout");
    assert_eq!(min.keys.len(), 1, "MIN should be a single key");
    assert_eq!(
        min.keys[0].action,
        tuke::KeyAction::Switch {
            to: "MAIN".to_string()
        }
    );
}

#[test]
fn mini_left_main_still_carries_a_letter_board() {
    // Splitting the board must not disturb its core: `MAIN` is the board a
    // hand lives on, so the letters stay on it whatever else moves away.
    let set = shipped_layout_set("default.jsonc");
    let main = set.get("MAIN").expect("MAIN layout");
    let has = |code: tuke::KeyCode| main.keys.iter().any(|k| code_of(k) == code);

    for c in 'a'..='z' {
        assert!(has(tuke::KeyCode::Char(c)), "MAIN is missing letter {c}");
    }
}

#[test]
fn mini_left_sub_carries_the_keys_main_gave_up() {
    // The digits and Esc left `MAIN` to stay small, and the second board is
    // where they must have landed: a split that dropped them would still
    // load, so this is what pins where they went.
    let set = shipped_layout_set("default.jsonc");
    let sub = set.get("SUB").expect("SUB layout");
    let has = |code: tuke::KeyCode| sub.keys.iter().any(|k| code_of(k) == code);

    for c in '0'..='9' {
        assert!(has(tuke::KeyCode::Char(c)), "SUB is missing digit {c}");
    }
    assert!(has(tuke::KeyCode::Escape), "SUB is missing Escape");
}

#[test]
fn mini_left_has_no_arrow_keys() {
    // The arrow cluster was taken off the board. A key that quietly came back
    // would widen the row it sits on, so the absence is what this pins.
    let set = shipped_layout_set("default.jsonc");

    for name in ["MAIN", "SUB", "MIN"] {
        let layout = set.get(name).expect("layout {name}");
        for key in &layout.keys {
            if let tuke::KeyAction::Send { code, .. } = key.action {
                assert!(
                    !matches!(
                        code,
                        tuke::KeyCode::Up
                            | tuke::KeyCode::Down
                            | tuke::KeyCode::Left
                            | tuke::KeyCode::Right
                    ),
                    "{name} carries arrow key {code}"
                );
            }
        }
    }
}

#[test]
fn a_layout_without_a_keyboard_pos_uses_the_corner() {
    let layout = tuke::Layout::load_from_file(write_temp(r#"[{"key": "a"}]"#))
        .expect("load a layout with no keyboard_pos");

    assert_eq!(layout.keyboard_pos, tuke::KeyboardPos::ORIGIN);
    assert_eq!(layout.keyboard_pos.col, 0);
    assert_eq!(layout.keyboard_pos.rows, 0);
}

#[test]
fn a_keyboard_pos_sets_where_the_keyboard_floats() {
    let layout = tuke::Layout::load_from_file(write_temp(
        r#"[
            {"keyboard_pos": {"col": 12, "rows": 2}},
            {"key": "a"}
        ]"#,
    ))
    .expect("load a layout with a keyboard_pos");

    assert_eq!(layout.keyboard_pos, tuke::KeyboardPos { col: 12, rows: 2 });
}

#[test]
fn a_keyboard_pos_carries_over_to_the_layouts_after_it() {
    // The entry is positional, like `base_position` and `default_size`: it
    // stays in force until another `keyboard_pos` replaces it, so one board can
    // pin several layouts to the same corner with a single entry.
    let set = tuke::LayoutSet::load_from_file(write_temp(
        r#"[
            {"layout": "first"},
            {"keyboard_pos": {"col": 5, "rows": 1}},
            {"key": "a"},
            {"layout": "second"},
            {"key": "b"}
        ]"#,
    ))
    .expect("load a set with one keyboard_pos");

    assert_eq!(
        set.get("first").expect("first").keyboard_pos,
        tuke::KeyboardPos { col: 5, rows: 1 }
    );
    assert_eq!(
        set.get("second").expect("second").keyboard_pos,
        tuke::KeyboardPos { col: 5, rows: 1 },
        "the second layout inherited the first's position"
    );
}

#[test]
fn a_later_keyboard_pos_replaces_an_earlier_one() {
    let set = tuke::LayoutSet::load_from_file(write_temp(
        r#"[
            {"layout": "first"},
            {"keyboard_pos": {"col": 5, "rows": 1}},
            {"key": "a"},
            {"layout": "second"},
            {"keyboard_pos": {"col": 0, "rows": 0}},
            {"key": "b"}
        ]"#,
    ))
    .expect("load a set with two keyboard_pos entries");

    assert_eq!(
        set.get("first").expect("first").keyboard_pos,
        tuke::KeyboardPos { col: 5, rows: 1 }
    );
    assert_eq!(
        set.get("second").expect("second").keyboard_pos,
        tuke::KeyboardPos::ORIGIN
    );
}

#[test]
fn a_keyboard_pos_missing_a_member_is_rejected() {
    let result = tuke::Layout::load_from_file(write_temp(
        r#"[
            {"keyboard_pos": {"col": 1}},
            {"key": "a"}
        ]"#,
    ));

    assert!(result.is_err(), "`rows` is required when `col` is given");
}
