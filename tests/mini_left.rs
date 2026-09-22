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

/// The lowest row any key or the preview extends to, in layout cells.
fn layout_rows(layout: &tuke::Layout) -> usize {
    layout
        .keys
        .iter()
        .map(|k| k.region.position.row + k.region.size.rows)
        .chain(
            layout
                .preview
                .iter()
                .map(|p| p.region.position.row + p.region.size.rows),
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
fn mini_left_is_taller_than_mini() {
    let left = layout_rows(&shipped_layout("mini-left.jsonc"));
    let mini = layout_rows(&shipped_layout("mini.jsonc"));

    // The extra height is the whole point: the digits are split over two rows
    // so the board uses vertical space a narrow screen leaves over.
    assert!(
        left > mini,
        "mini-left is {left} rows tall, no taller than mini's {mini}"
    );
}

#[test]
fn mini_left_carries_the_keys_a_shell_needs() {
    let layout = shipped_layout("mini-left.jsonc");
    let has = |code: tuke::KeyCode| layout.keys.iter().any(|k| k.code == code);

    for c in 'a'..='z' {
        assert!(has(tuke::KeyCode::Char(c)), "missing letter {c}");
    }
    for c in '0'..='9' {
        assert!(has(tuke::KeyCode::Char(c)), "missing digit {c}");
    }
    for c in ['.', '-', '_', '/', '?', '"', '(', ')'] {
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
                a.code, a.region, b.code, b.region
            );
        }
    }
}

#[test]
fn mini_left_labels_fit_their_keys() {
    let layout = shipped_layout("mini-left.jsonc");

    for key in &layout.keys {
        let label = key.code.to_string();
        let interior = key.region.size.cols.saturating_sub(2);
        assert!(
            label.chars().count() <= interior,
            "label {label:?} needs {} columns but key {:?} has {interior}",
            label.chars().count(),
            key.code
        );
    }
}
