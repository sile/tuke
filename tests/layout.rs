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
/// A key is either a send key or a switch key; these tests only build send
/// keys, so this unwraps the code rather than threading a `Result` through
/// every assertion.
fn code_of(key: &tuke::Key) -> tuke::KeyCode {
    match key.action {
        tuke::KeyAction::Send { code, .. } => code,
        tuke::KeyAction::Switch { .. } => panic!("expected a send key, got a switch key"),
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

    // The default layout has the preview row and the QWERTY letters.
    assert!(layout.preview.is_some());
    assert!(
        layout
            .keys
            .iter()
            .any(|k| code_of(k) == tuke::KeyCode::Char('q'))
    );
    assert!(
        layout
            .keys
            .iter()
            .any(|k| code_of(k) == tuke::KeyCode::Char(' '))
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
fn the_shipped_default_layout_is_one_layout_named_default() {
    // The shipped layouts have no `{"layout": …}` entry, so this is the shape
    // the default file takes: one unnamed layout holding every key.
    let set: tuke::LayoutSet = tuke::LayoutSet::default();

    assert_eq!(set.layouts().len(), 1);
    assert_eq!(set.layouts()[0].name, "default");
    assert!(set.first().preview.is_some());
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
fn mini_fits_in_eighty_columns() {
    let layout = shipped_layout("mini.jsonc");

    // The point of this layout is that it is usable on a narrow screen: an
    // 80-column terminal must be able to show it without cropping.
    assert!(
        layout_cols(&layout) <= 80,
        "mini is {} columns wide, over the 80-column budget",
        layout_cols(&layout)
    );
}

#[test]
fn mini_carries_the_keys_a_shell_needs() {
    let layout = shipped_layout("mini.jsonc");
    let has = |code: tuke::KeyCode| layout.keys.iter().any(|k| code_of(k) == code);

    for c in 'a'..='z' {
        assert!(has(tuke::KeyCode::Char(c)), "missing letter {c}");
    }
    for c in '0'..='9' {
        assert!(has(tuke::KeyCode::Char(c)), "missing digit {c}");
    }
    for c in ['.', '-', '_', '/', '?', '~', '"', '(', ')'] {
        assert!(has(tuke::KeyCode::Char(c)), "missing symbol {c}");
    }
    for code in [
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Escape,
        tuke::KeyCode::Tab,
        tuke::KeyCode::Enter,
        tuke::KeyCode::Backspace,
        tuke::KeyCode::Char(' '),
        tuke::KeyCode::Left,
        tuke::KeyCode::Down,
        tuke::KeyCode::Up,
        tuke::KeyCode::Right,
    ] {
        assert!(has(code), "missing {code}");
    }

    // The arrows form the inverted T: Up sits directly above Down, and Left
    // and Right sit either side of Down on its own row.
    let arrow = |code: tuke::KeyCode| {
        layout
            .keys
            .iter()
            .find(|k| code_of(k) == code)
            .map(|k| k.region)
            .unwrap_or_else(|| panic!("missing {code}"))
    };
    let up = arrow(tuke::KeyCode::Up);
    let down = arrow(tuke::KeyCode::Down);
    let left = arrow(tuke::KeyCode::Left);
    let right = arrow(tuke::KeyCode::Right);

    assert_eq!(up.position.col, down.position.col, "Up is not above Down");
    assert!(down.position.row > up.position.row, "Down is not below Up");
    assert_eq!(
        left.position.row, down.position.row,
        "Left is not on Down's row"
    );
    assert_eq!(
        right.position.row, down.position.row,
        "Right is not on Down's row"
    );
    assert!(
        left.position.col + left.size.cols <= down.position.col,
        "Left is not left of Down"
    );
    assert!(
        right.position.col >= down.position.col + down.size.cols,
        "Right is not right of Down"
    );

    // Shift and Alt are deliberately absent: a compact layout carries only the
    // modifier a shell cannot do without.
    assert!(!has(tuke::KeyCode::Shift), "mini should not carry Shift");
    assert!(!has(tuke::KeyCode::Alt), "mini should not carry Alt");
}

#[test]
fn mini_has_no_overlapping_keys() {
    let layout = shipped_layout("mini.jsonc");

    // Overlapping keys would make a hit test ambiguous, and the JSONC cursor
    // rules already place them, so a collision means the layout moved a key by
    // hand on top of another.
    for (i, a) in layout.keys.iter().enumerate() {
        for b in &layout.keys[i + 1..] {
            let separated = a.region.position.col + a.region.size.cols <= b.region.position.col
                || b.region.position.col + b.region.size.cols <= a.region.position.col
                || a.region.position.row + a.region.size.rows <= b.region.position.row
                || b.region.position.row + b.region.size.rows <= a.region.position.row;
            assert!(
                separated,
                "keys {:?} at {:?} and {:?} at {:?} overlap",
                code_of(a),
                a.region,
                code_of(b),
                b.region
            );
        }
    }
}

#[test]
fn mini_labels_fit_their_keys() {
    let layout = shipped_layout("mini.jsonc");

    // The renderer crops a label that is too long for its key, so a cramped
    // label is not an error - but `BSpace` in a three-column key would read
    // `BSp`, which is a layout bug rather than a rendering one.
    for key in &layout.keys {
        let label = code_of(key).to_string();
        let interior = key.region.size.cols.saturating_sub(2);
        assert!(
            label.chars().count() <= interior,
            "label {label:?} needs {} columns but key {:?} has {interior}",
            label.chars().count(),
            code_of(key)
        );
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
fn mini_left_switches_between_main_and_min() {
    // The shipped one-handed board carries two layouts: `MAIN`, the full board
    // it starts on, and `MIN`, a one-key board whose only job is to put
    // `MAIN` back. Both directions must resolve, or the file would not load
    // (an unknown target is rejected), so this pins the shape the file is
    // meant to have.
    let set = shipped_layout_set("mini-left.jsonc");

    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MAIN", "MIN"]);
    assert_eq!(set.first_name(), "MAIN", "the full board starts up");

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
                    tuke::KeyAction::Send { .. } => None,
                })
        })
        .collect();

    assert_eq!(
        switches,
        [
            ("MAIN".to_string(), "MIN".to_string()),
            ("MIN".to_string(), "MAIN".to_string()),
        ],
        "MAIN goes to MIN and MIN goes back to MAIN"
    );

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
    // Adding the switch to `MIN` must not disturb the board itself.
    let set = shipped_layout_set("mini-left.jsonc");
    let main = set.get("MAIN").expect("MAIN layout");
    let has = |code: tuke::KeyCode| main.keys.iter().any(|k| code_of(k) == code);

    for c in 'a'..='z' {
        assert!(has(tuke::KeyCode::Char(c)), "MAIN is missing letter {c}");
    }
    for c in '0'..='9' {
        assert!(has(tuke::KeyCode::Char(c)), "MAIN is missing digit {c}");
    }
}
