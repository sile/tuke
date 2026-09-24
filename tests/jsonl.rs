//! Tests for JSON Lines layout loading and the error it reports.
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
    std::env::temp_dir().join(format!("tuke-jsonl-{tag}-{}.jsonl", std::process::id()))
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
        tuke::KeyAction::Shortcut { .. } => panic!("expected a send key, got a shortcut key"),
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
fn one_entry_per_line_is_read_in_order() {
    // The point of JSON Lines: one entry per line, no enclosing array.
    let layout = load(
        "one-per-line",
        "{\"key\": \"a\", \"size\": {\"width\": 3, \"height\": 3}}\n\
         {\"key\": \"b\", \"size\": {\"width\": 3, \"height\": 3}}\n",
    )
    .expect("two one-line entries parse");

    assert_eq!(layout.keys.len(), 2);
    assert_eq!(code_of(&layout.keys[0]), tuke::KeyCode::Char('a'));
    assert_eq!(code_of(&layout.keys[1]), tuke::KeyCode::Char('b'));
}

#[test]
fn blank_lines_are_skipped() {
    let layout = load(
        "blank-lines",
        "\n{\"key\": \"a\", \"size\": {\"width\": 3, \"height\": 3}}\n\n\n",
    )
    .expect("blank lines carry no entry");

    assert_eq!(layout.keys.len(), 1);
}

#[test]
fn a_hash_comment_line_is_skipped() {
    // In JSONL a `#` at the start of a line is a comment; a `#` inside an
    // entry (the key code `"#"`) is untouched.
    let layout = load(
        "hash-comment",
        "# a comment line\n\
         {\"key\": \"#\", \"size\": {\"width\": 3, \"height\": 3}}\n",
    )
    .expect("a comment line and a `#` key both work");

    assert_eq!(layout.keys.len(), 1);
    assert_eq!(code_of(&layout.keys[0]), tuke::KeyCode::Char('#'));
}

#[test]
fn a_slash_slash_comment_is_not_a_comment() {
    // `//` is JSONC, not JSONL; a layout that uses it is a parse error rather
    // than something silently ignored.
    let error = load(
        "slash-comment",
        "// a comment\n{\"key\": \"a\", \"size\": {\"width\": 3, \"height\": 3}}\n",
    )
    .expect_err("`//` is not a JSONL comment");

    assert!(!error.to_string().is_empty());
}

#[test]
fn a_trailing_comma_within_a_line_is_rejected() {
    // Each entry is strict JSON, so JSONC's conveniences do not leak in: a
    // trailing comma inside an entry is a parse error, not quietly accepted.
    let error = load(
        "trailing-comma",
        "{\"key\": \"a\", \"size\": {\"width\": 3, \"height\": 3},}\n",
    )
    .expect_err("a trailing comma is not strict JSON");

    assert!(!error.to_string().is_empty());
}

#[test]
fn a_parse_error_reports_the_file_line_not_the_first_line() {
    // Each line is parsed on its own, so the parse error's own line number
    // would be 1. The report must name the entry's real line in the file.
    let error = load(
        "line-number",
        "{\"key\": \"a\", \"size\": {\"width\": 3, \"height\": 3}}\n\
         {\"key\": \"b\", \"size\": {\"width\": 3, \"height\": 3}}\n\
         not json at all\n",
    )
    .expect_err("the third line is not an entry");

    let message = error.to_string();
    assert!(
        message.contains(":3:"),
        "the report should name line 3: {message}"
    );
}

#[test]
fn a_key_without_a_code_is_rejected() {
    let error = load("no-code", "{\"size\": {\"width\": 3, \"height\": 3}}\n")
        .expect_err("a key needs a code to send");

    assert!(!error.to_string().is_empty());
}

#[test]
fn an_unknown_key_code_names_the_bad_value() {
    let error = load("unknown-code", "{\"key\": \"F1\"}\n").expect_err("F1 is not a tuke key code");

    // The report quotes the value the user wrote, not just "invalid".
    let message = error.to_string();
    assert!(message.contains("key code"), "unhelpful message: {message}");
}

#[test]
fn a_non_numeric_size_is_rejected() {
    let error = load(
        "non-numeric",
        "{\"key\": \"a\", \"size\": {\"width\": \"wide\", \"height\": 3}}\n",
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
        "{\"key\": \"a\", \"size\": {\"width\": 0, \"height\": 3}}\n",
    )
    .expect_err("width 0");

    assert!(!error.to_string().is_empty());
}

#[test]
fn a_switch_to_an_unknown_layout_is_rejected() {
    let error = load(
        "unknown-switch",
        "{\"layout\": \"MAIN\"}\n\
         {\"key\": {\"switch_to\": \"NOPE\"}, \"size\": {\"width\": 3, \"height\": 3}}\n",
    )
    .expect_err("NOPE is not defined in the file");

    let message = error.to_string();
    assert!(
        message.contains("NOPE"),
        "should name the target: {message}"
    );
}

#[test]
fn the_default_layout_parses_from_the_embedded_file() {
    // `Layout::default()` loads `layouts/default.jsonl`; if that file drifted
    // out of the format, every run with no layout file would fail. The file
    // declares several boards, and the one shown at startup is the first.
    let layout = tuke::Layout::default();

    assert!(
        layout.keys.len() >= 20,
        "the default layout's first board is a full keyboard, got {}",
        layout.keys.len()
    );

    let set = tuke::LayoutSet::default();
    let names: Vec<&str> = set.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["MAIN", "SUB", "MIN"]);
    assert_eq!(set.first_name(), "MAIN", "the everyday board starts up");
}
