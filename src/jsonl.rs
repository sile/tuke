//! JSON Lines loading.
//!
//! A layout file is JSON Lines: one JSON object per line, with `#` starting an
//! ignored line and blank lines skipped. Every line is parsed as strict JSON:
//! a single entry cannot be spread over several lines, and a line keeps no
//! comment or trailing comma, so two files that say the same thing look the
//! same, and the line a parse error reports is the line the entry is on.
//!
//! This module only turns `text` into owned per-line values and their line
//! numbers; what those entries mean is the caller's ([`crate::layout`]).

use std::num::NonZeroUsize;

use crate::error::{Error, Result};

/// One parsed entry line.
pub(crate) struct Entry<'text> {
    /// The line's JSON, kept alive so its value can be read.
    pub(crate) json: nojson::RawJson<'text>,
    /// The 1-based line number the entry is on.
    pub(crate) line: NonZeroUsize,
}

/// Parses `text` as JSONL, using `name` in any error message.
///
/// Every non-blank, non-comment line becomes one [`Entry`], in file order. A
/// line that fails to parse is reported against that line: [`Error::Json`]
/// carries the line's number so [`crate::Error`]'s `Display` prints the file's
/// line, not the reset-at-1 line of the single line that was parsed.
pub(crate) fn parse_entries<'text>(name: &str, text: &'text str) -> Result<Vec<Entry<'text>>> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = NonZeroUsize::new(index + 1).expect("line numbers start at 1");
        if is_ignored(line) {
            continue;
        }
        let json = nojson::RawJson::parse(line)
            .map_err(|error| Error::json(name, line, Some(line_number), error))?;
        entries.push(Entry {
            json,
            line: line_number,
        });
    }
    Ok(entries)
}

/// Whether a line is a comment or blank and so carries no entry.
///
/// A `#` only starts a comment where the line has nothing but whitespace
/// before it, so a `#` inside an object (as the key code `"#"`) is untouched.
fn is_ignored(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty() || trimmed.starts_with('#')
}
