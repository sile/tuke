//! Tests for the layout model: JSONC parsing and key code mapping.

/// Parses a single JSON string literal as a `KeyCode`.
///
/// The layout file writes a key code as a quoted string, so the test parses
/// `"<code>"` the same way the layout loader does.
fn parse_code(literal: &str) -> Result<tuke::KeyCode, nojson::JsonParseError> {
    let (json, _) = nojson::RawJson::parse_jsonc(literal).expect("literal is valid JSONC");
    tuke::KeyCode::try_from(json.value())
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
            .any(|k| k.code == tuke::KeyCode::Char('q'))
    );
    assert!(
        layout
            .keys
            .iter()
            .any(|k| k.code == tuke::KeyCode::Char(' '))
    );
}

#[test]
fn layout_from_a_file_matches_the_text() {
    let path = std::env::temp_dir().join(format!("tuke-layout-test-{}.jsonc", std::process::id()));
    std::fs::write(
        &path,
        r#"[{"key": "a", "size": {"width": 5, "height": 5}}]"#,
    )
    .expect("write layout file");

    let layout = tuke::Layout::load_from_file(&path).expect("parse layout file");
    let _ = std::fs::remove_file(&path);

    assert_eq!(layout.keys.len(), 1);
    assert_eq!(layout.keys[0].code, tuke::KeyCode::Char('a'));
    assert_eq!(
        layout.keys[0].region.size,
        tuinix::Size { rows: 5, cols: 5 }
    );
    assert!(layout.preview.is_none());
}

#[test]
fn a_size_below_the_minimum_is_rejected() {
    let path = std::env::temp_dir().join(format!("tuke-layout-min-{}.jsonc", std::process::id()));
    std::fs::write(
        &path,
        r#"[{"key": "a", "size": {"width": 2, "height": 5}}]"#,
    )
    .expect("write layout file");

    let result = tuke::Layout::load_from_file(&path);
    let _ = std::fs::remove_file(&path);

    assert!(result.is_err(), "a 2-column key is too small to draw");
}
