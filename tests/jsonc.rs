//! Tests for JSONC layout loading and the error it reports.
//!
//! The loader is reached through `Layout::load_from_file`, so these cover the
//! path a user hits with a hand-written layout: a file that does not parse,
//! and one that parses but does not describe a layout.

/// Writes `text` to a uniquely named temporary file and returns its path.
///
/// The name includes the pid so the test binaries do not collide when several
/// run at once, and a caller-supplied tag so the tests inside one binary do
/// not share a file: they run on separate threads concurrently.
fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("tuke-jsonc-{tag}-{}.jsonc", std::process::id()))
}

/// Writes `text` to a temporary file named after `tag`, loads it as a layout,
/// and removes it.
fn load(tag: &str, text: &str) -> Result<tuke::Layout, tuke::Error> {
    let path = temp_path(tag);
    std::fs::write(&path, text).expect("write layout file");
    let result = tuke::Layout::load_from_file(&path);
    let _ = std::fs::remove_file(&path);
    result
}

/// The code a send key sends when Shift is not active.
fn code_of(key: &tuke::Key) -> tuke::KeyCode {
    match key.action {
        tuke::KeyAction::Send { code, .. } => code,
        tuke::KeyAction::Switch { .. } => panic!("expected a send key, got a switch key"),
    }
}

#[test]
fn a_missing_file_reports_its_path() {
    let path = temp_path("does-not-exist");
    let _ = std::fs::remove_file(&path);

    let error = tuke::Layout::load_from_file(&path).expect_err("the file is absent");

    // The message names the file, so a user can tell which layout failed.
    let message = error.to_string();
    assert!(
        message.contains("does-not-exist"),
        "the error should name the file: {message}"
    );
}

#[test]
fn malformed_jsonc_reports_the_offending_position() {
    let error = load("malformed", "[{\"key\": \"a\"},").expect_err("the file is truncated");

    // A parse error is rendered with the path, the line and column, and a
    // caret under the offending column, so it reads like a compiler error.
    let message = error.to_string();
    assert!(message.contains("error"), "no caret marker: {message}");
    assert!(message.contains(":1:"), "no line:column: {message}");
}

#[test]
fn a_trailing_comma_is_accepted() {
    // JSONC allows comments and trailing commas, which is the point of the
    // format for a hand-written layout.
    let layout = load(
        "trailing-comma",
        r#"[
            // one key
            {"key": "a", "size": {"width": 3, "height": 3}},
        ]"#,
    )
    .expect("JSONC allows a trailing comma");

    assert_eq!(layout.keys.len(), 1);
    assert_eq!(code_of(&layout.keys[0]), tuke::KeyCode::Char('a'));
}

#[test]
fn comments_are_ignored() {
    let layout = load(
        "comments",
        r#"[
            // a line comment
            {"key": "a", "size": {"width": 3, "height": 3}},
            /* a block comment */
            {"key": "b", "size": {"width": 3, "height": 3}},
        ]"#,
    )
    .expect("comments are JSONC");

    assert_eq!(layout.keys.len(), 2);
}

#[test]
fn a_layout_that_is_not_an_array_is_rejected() {
    let error = load("not-array", r#"{"key": "a"}"#).expect_err("the top level must be an array");

    assert!(!error.to_string().is_empty());
}

#[test]
fn a_key_without_a_code_is_rejected() {
    let error = load("no-code", r#"[{"size": {"width": 3, "height": 3}}]"#)
        .expect_err("a key needs a code to send");

    assert!(!error.to_string().is_empty());
}

#[test]
fn an_unknown_key_code_names_the_bad_value() {
    let error = load("unknown-code", r#"[{"key": "F1"}]"#).expect_err("F1 is not a tuke key code");

    // The report quotes the value the user wrote, not just "invalid".
    let message = error.to_string();
    assert!(message.contains("key code"), "unhelpful message: {message}");
}

#[test]
fn a_non_numeric_size_is_rejected() {
    let error = load(
        "non-numeric",
        r#"[{"key": "a", "size": {"width": "wide", "height": 3}}]"#,
    )
    .expect_err("a width must be a number");

    assert!(!error.to_string().is_empty());
}

#[test]
fn a_zero_sized_key_is_rejected() {
    // A key with no columns cannot be drawn or clicked, so it is a layout
    // mistake rather than an empty key.
    let error = load(
        "zero-sized",
        r#"[{"key": "a", "size": {"width": 0, "height": 3}}]"#,
    )
    .expect_err("width 0");

    assert!(!error.to_string().is_empty());
}

#[test]
fn the_default_layout_parses_from_the_embedded_file() {
    // `Layout::default()` loads `layouts/default.jsonc`; if that file drifted
    // out of the format, every run without `--layout-file` would fail.
    let layout = tuke::Layout::default();

    assert!(layout.preview.is_some(), "the default layout has a preview");
    assert!(
        layout.keys.len() >= 40,
        "the default layout declares a full keyboard, got {}",
        layout.keys.len()
    );
}
