//! Tests for the shipped one-handed left layout.

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

/// Loads a layout that ships with tuke, by file name under `layouts/`.
fn shipped_layout(name: &str) -> tuke::Layout {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("layouts")
        .join(name);
    tuke::Layout::load_from_file(&path)
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()))
}

/// Loads a shipped layout file as a set of named layouts.
///
/// The one-handed board is split over several layouts (`MAIN`, `SUB`, `MIN`),
/// so a test that asks what the file carries as a whole has to look at all of
/// them rather than only the first one the file declares.
fn shipped_layout_set(name: &str) -> tuke::LayoutSet {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("layouts")
        .join(name);
    tuke::LayoutSet::load_from_file(&path)
        .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()))
}

/// Whether any of the set's layouts carries the code.
fn any_has(set: &tuke::LayoutSet, code: tuke::KeyCode) -> bool {
    set.layouts()
        .iter()
        .flat_map(|named| named.layout.keys.iter())
        .any(|key| send_code(key) == Some(code))
}

/// The code a send key sends when Shift is not active, or `None` for a key
/// that does not send a code (a layout switch).
///
/// The shipped board carries a switch key (the way out to `MIN`), so tests that
/// look for a code must step over it rather than unwrap.
fn send_code(key: &tuke::Key) -> Option<tuke::KeyCode> {
    match key.action {
        tuke::KeyAction::Send { code, .. } => Some(code),
        tuke::KeyAction::Switch { .. } => None,
    }
}

/// What a key is, for a failure message: its code, or where a switch goes.
fn describe(key: &tuke::Key) -> String {
    match &key.action {
        tuke::KeyAction::Send { code, .. } => code.to_string(),
        tuke::KeyAction::Switch { to } => format!("switch to {to}"),
    }
}

#[test]
fn mini_left_letter_and_thumb_rows_start_at_the_left_edge() {
    let layout = shipped_layout("mini-left.jsonc");

    // The letter rows and the thumb row sit flush against the left edge, so a
    // left hand resting on the board reaches every key without reaching right.
    for code in [
        tuke::KeyCode::Char('q'),
        tuke::KeyCode::Char('a'),
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Backspace,
    ] {
        let key = layout
            .keys
            .iter()
            .find(|k| send_code(k) == Some(code))
            .unwrap_or_else(|| panic!("missing {code}"));
        assert_eq!(
            key.region.position.col, 0,
            "{code} starts at column {}, not the left edge",
            key.region.position.col
        );
    }
}

#[test]
fn mini_left_fits_a_narrow_screen() {
    let layout = shipped_layout("mini-left.jsonc");

    // The point of a one-handed board is that it stays out of the other hand's
    // way, so it must not reach the middle of an 80-column terminal.
    assert!(
        layout_cols(&layout) <= 70,
        "mini-left is {} columns wide, over the one-handed budget",
        layout_cols(&layout)
    );
}

#[test]
fn mini_left_keeps_its_boards_no_wider_than_mini() {
    let set = shipped_layout_set("mini-left.jsonc");
    let mini = shipped_layout("mini.jsonc");

    // Splitting the board over `MAIN` and `SUB` must not make either one wider
    // than the compact board it is meant to be a one-handed alternative to:
    // the point of the split is to stay reachable by one hand, so a wider
    // board would defeat it.
    for named in set.layouts() {
        assert!(
            layout_cols(&named.layout) <= layout_cols(&mini),
            "{} is {} columns wide, over mini's {}",
            named.name,
            layout_cols(&named.layout),
            layout_cols(&mini)
        );
    }
}

#[test]
fn mini_left_carries_the_keys_a_shell_needs() {
    // The everyday board is split over `MAIN` and `SUB`: `MAIN` keeps the
    // letters and the keys a hand wants constantly, and `SUB` takes the rest.
    // What matters is that the file as a whole still carries what a shell
    // needs, so this looks at every layout rather than only `MAIN`.
    let set = shipped_layout_set("mini-left.jsonc");

    for c in 'a'..='z' {
        assert!(any_has(&set, tuke::KeyCode::Char(c)), "missing letter {c}");
    }
    for c in '0'..='9' {
        assert!(any_has(&set, tuke::KeyCode::Char(c)), "missing digit {c}");
    }
    for c in ['.', '-', '_', '/', '?', '"', '(', ')'] {
        assert!(any_has(&set, tuke::KeyCode::Char(c)), "missing symbol {c}");
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
        assert!(any_has(&set, code), "missing {code}");
    }
}

#[test]
fn mini_left_main_keeps_the_everyday_keys() {
    // The split earns its keep only if `MAIN` is the board a hand lives on: it
    // must carry the letters and the keys pressed constantly, and leave the
    // rest to `SUB`. Moving the letters away would make the everyday board
    // useless even though the file as a whole still carried them.
    let set = shipped_layout_set("mini-left.jsonc");
    let main = set.get("MAIN").expect("MAIN layout");
    let has = |code: tuke::KeyCode| main.keys.iter().any(|k| send_code(k) == Some(code));

    for c in 'a'..='z' {
        assert!(has(tuke::KeyCode::Char(c)), "MAIN is missing letter {c}");
    }
    for code in [
        tuke::KeyCode::Ctrl,
        tuke::KeyCode::Tab,
        tuke::KeyCode::Enter,
        tuke::KeyCode::Backspace,
        tuke::KeyCode::Char(' '),
    ] {
        assert!(has(code), "MAIN is missing {code}");
    }

    // What moved: the digits and the arrows are `SUB`'s job now, so `MAIN`
    // must not carry them and grow back to the size the split was meant to
    // shrink.
    for code in [
        tuke::KeyCode::Char('0'),
        tuke::KeyCode::Escape,
        tuke::KeyCode::Left,
        tuke::KeyCode::Down,
        tuke::KeyCode::Up,
        tuke::KeyCode::Right,
    ] {
        assert!(!has(code), "MAIN should have given {code} up to SUB");
    }
}

#[test]
fn mini_left_has_no_overlapping_keys() {
    let layout = shipped_layout("mini-left.jsonc");

    for (i, a) in layout.keys.iter().enumerate() {
        for b in &layout.keys[i + 1..] {
            let separated = a.region.position.col + a.region.size.cols <= b.region.position.col
                || b.region.position.col + b.region.size.cols <= a.region.position.col
                || a.region.position.row + a.region.size.rows <= b.region.position.row
                || b.region.position.row + b.region.size.rows <= a.region.position.row;
            assert!(
                separated,
                "keys {:?} at {:?} and {:?} at {:?} overlap",
                describe(a),
                a.region,
                describe(b),
                b.region
            );
        }
    }
}

#[test]
fn mini_left_labels_fit_their_keys() {
    let layout = shipped_layout("mini-left.jsonc");

    for key in &layout.keys {
        // A switch key is labelled with where it goes, so its label is the
        // destination name; every other key shows its code.
        let label = match &key.action {
            tuke::KeyAction::Send { code, .. } => code.to_string(),
            tuke::KeyAction::Switch { to } => to.clone(),
        };
        let interior = key.region.size.cols.saturating_sub(2);
        assert!(
            label.chars().count() <= interior,
            "label {label:?} needs {} columns but key {:?} has {interior}",
            label.chars().count(),
            describe(key)
        );
    }
}
