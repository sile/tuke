use std::path::PathBuf;

/// The crate-wide result type.
pub type Result<T> = std::result::Result<T, Error>;

/// The error type used throughout this crate.
#[derive(Debug)]
pub enum Error {
    /// An I/O operation failed (terminal device, file, or child process).
    Io(std::io::Error),

    /// A JSONC layout file could not be parsed.
    Json {
        /// Path of the file that failed to parse.
        path: PathBuf,
        /// The whole text of the file, kept so the error can quote the
        /// offending line.
        text: String,
        /// The underlying parse error.
        error: nojson::JsonParseError,
    },

    /// A failure that does not fit any other variant.
    Message(String),
}

impl Error {
    /// Makes a [`Error::Message`] from anything printable.
    pub fn message<T: std::fmt::Display>(message: T) -> Self {
        Self::Message(message.to_string())
    }

    /// Makes a [`Error::Json`], owning the offending text.
    pub fn json(path: &str, text: &str, error: nojson::JsonParseError) -> Self {
        Self::Json {
            path: PathBuf::from(path),
            text: text.to_owned(),
            error,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json { path, text, error } => format_json_error(f, path, error, text),
            Self::Message(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json { error, .. } => Some(error),
            Self::Message(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<std::process::ExitStatus> for Error {
    fn from(status: std::process::ExitStatus) -> Self {
        Self::Message(format!("process exited with {status}"))
    }
}

fn format_json_error(
    f: &mut std::fmt::Formatter<'_>,
    path: &std::path::Path,
    error: &nojson::JsonParseError,
    text: &str,
) -> std::fmt::Result {
    let (line_num, column_num) = error
        .get_line_and_column_numbers(text)
        .unwrap_or((std::num::NonZeroUsize::MIN, std::num::NonZeroUsize::MIN));

    let line = error.get_line(text).unwrap_or("");
    let (display_line, display_column) = format_line_around_position(line, column_num.get());
    writeln!(f, "{error}")?;
    writeln!(f, "--> {}:{line_num}:{column_num}", path.display())?;
    writeln!(f, "{line_num:4} |{display_line}")?;
    writeln!(f, "     |{:>column$} error", "^", column = display_column)?;
    Ok(())
}

fn format_line_around_position(line: &str, column_pos: usize) -> (String, usize) {
    const MAX_ERROR_LINE_CHARS: usize = 80;

    let chars: Vec<char> = line.chars().collect();
    let max_context = MAX_ERROR_LINE_CHARS / 2;

    let error_pos = column_pos.saturating_sub(1).min(chars.len());
    let start_pos = error_pos.saturating_sub(max_context);
    let end_pos = (error_pos + max_context + 1).min(chars.len());

    let mut result = String::new();
    let mut new_column_pos = error_pos - start_pos + 1;

    if start_pos > 0 {
        result.push_str("...");
        new_column_pos += 3;
    }

    result.push_str(&chars[start_pos..end_pos].iter().collect::<String>());

    if end_pos < chars.len() {
        result.push_str("...");
    }

    (result, new_column_pos)
}
