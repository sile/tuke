use std::fmt::Write;
use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Layout {
    pub keys: Vec<Key>,
}

impl Layout {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        crate::jsonc::load_file(path).or_fail()
    }
}

impl Default for Layout {
    fn default() -> Self {
        crate::jsonc::load_str("default.json", include_str!("../default-layout.jsonc"))
            .expect("bug")
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Layout {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let mut keys = Vec::new();
        let mut next_newline_rows = 1;
        let mut last_size = tuinix::TerminalSize { rows: 3, cols: 3 };
        let mut position = tuinix::TerminalPosition::ZERO;
        for key_value in value.to_array()? {
            if let Some(blank_count) = key_value.to_member("blank")?.get() {
                let count: std::num::NonZeroUsize = blank_count.try_into()?;
                position.col += count.get();
                continue;
            }
            if let Some(newline_count) = key_value.to_member("newline")?.get() {
                let count: std::num::NonZeroUsize = newline_count.try_into()?;
                position.col = 0;
                position.row += next_newline_rows - 1 + count.get();
                next_newline_rows = 1;
                continue;
            }

            let key = Key::parse(key_value, position, last_size)?;

            last_size = key.region.size;
            position = key.region.top_right();
            position.col += 1;
            next_newline_rows = next_newline_rows.max(key.region.size.rows);

            keys.push(key);
        }
        Ok(Self { keys })
    }
}

#[derive(Debug, Clone)]
pub struct Key {
    pub code: KeyCode,
    pub shift_code: KeyCode,
    pub region: tuinix::TerminalRegion,
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        position: tuinix::TerminalPosition,
        last_size: tuinix::TerminalSize,
    ) -> Result<Self, nojson::JsonParseError> {
        let code: KeyCode = value.to_member("key")?.required()?.try_into()?;

        let shift_code = if let Some(shift) = value.to_member("shift")?.get() {
            shift.try_into()?
        } else {
            code.default_shift_code()
        };

        let size = value
            .to_member("size")?
            .map(parse_size)?
            .unwrap_or(last_size);

        let region = tuinix::TerminalRegion { position, size };

        Ok(Self {
            code,
            shift_code,
            region,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    // Normal Key Codes (can be sent via `tmux send-keys`)
    Char(char),
    Shift,
    Ctrl,
    Alt,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
    Delete,
    Tab,

    // Special Key Codes
    Quit,
    DisplayPanes,
    SelectPane { index: usize },
    ShowCursor,
    CopyMode,
    Paste,
}

impl KeyCode {
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Shift | Self::Ctrl | Self::Alt)
    }

    pub fn is_special(self) -> bool {
        matches!(
            self,
            Self::Quit
                | Self::DisplayPanes
                | Self::SelectPane { .. }
                | Self::ShowCursor
                | Self::CopyMode
                | Self::Paste
        )
    }

    pub fn default_shift_code(self) -> Self {
        match self {
            Self::Char(c) => Self::Char(c.to_ascii_uppercase()),
            other => other,
        }
    }
}

impl std::fmt::Display for KeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Normal - tmux compatible notation
            Self::Char(c) => write!(f, "{c}"),
            Self::Shift => write!(f, "S-"),
            Self::Ctrl => write!(f, "C-"),
            Self::Alt => write!(f, "M-"),
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Enter => write!(f, "Enter"),
            Self::Backspace => write!(f, "BSpace"),
            Self::Delete => write!(f, "Delete"),
            Self::Tab => write!(f, "Tab"),

            // Special
            Self::Quit => write!(f, "Quit"),
            Self::DisplayPanes => write!(f, "Panes"),
            Self::SelectPane { index } => write!(f, "Pane{index}"),
            Self::ShowCursor => write!(f, "Cursor"),
            Self::CopyMode => write!(f, "Copy"),
            Self::Paste => write!(f, "Paste"),
        }
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for KeyCode {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        match value.to_unquoted_string_str()?.as_ref() {
            // Special
            "Quit" => Ok(Self::Quit),
            "Panes" => Ok(Self::DisplayPanes),
            s if s.starts_with("Pane") => {
                let index = s[4..].parse().map_err(|e| value.invalid(e))?;
                Ok(Self::SelectPane { index })
            }
            "Cursor" => Ok(Self::ShowCursor),
            "Copy" => Ok(Self::CopyMode),
            "Paste" => Ok(Self::Paste),

            // Normal
            "S-" => Ok(Self::Shift),
            "C-" => Ok(Self::Ctrl),
            "M-" => Ok(Self::Alt),
            "Up" => Ok(Self::Up),
            "Down" => Ok(Self::Down),
            "Left" => Ok(Self::Left),
            "Right" => Ok(Self::Right),
            "Enter" => Ok(Self::Enter),
            "BSpace" => Ok(Self::Backspace),
            "Delete" => Ok(Self::Delete),
            "Tab" => Ok(Self::Tab),
            s => {
                if let Some(c) = s.chars().next()
                    && s.len() == 1
                    && matches!(c, 'a'..='z' | '0'..='9' | '!'..='~' | ' ')
                {
                    Ok(Self::Char(c))
                } else {
                    Err(value.invalid("unknown key code"))
                }
            }
        }
    }
}

fn parse_size(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<tuinix::TerminalSize, nojson::JsonParseError> {
    let width_value = value.to_member("width")?.required()?;
    let width = width_value.try_into()?;
    if width < 3 {
        return Err(width_value.invalid("width must be at least 3"));
    }

    let height_value = value.to_member("height")?.required()?;
    let height = height_value.try_into()?;
    if height < 3 {
        return Err(height_value.invalid("height must be at least 3"));
    }

    Ok(tuinix::TerminalSize {
        rows: height,
        cols: width,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPressState {
    Neutral,
    Activated,
    OneshotActivated,
    Pressed,
}

#[derive(Debug, Clone)]
pub struct KeyState {
    pub key: Key,
    pub press: KeyPressState,
    pub selected: bool,
}

impl KeyState {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            press: KeyPressState::Neutral,
            selected: false,
        }
    }

    pub fn to_frame(&self, shift: bool) -> orfail::Result<tuinix::TerminalFrame> {
        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.key.region.size);

        let width = self.key.region.size.cols;
        let height = self.key.region.size.rows;

        let style = tuinix::TerminalStyle::new();
        let style = match self.press {
            KeyPressState::Neutral => style,
            KeyPressState::Pressed => style.bold(),
            KeyPressState::Activated => style.italic().reverse(),
            KeyPressState::OneshotActivated => style.italic(),
        };
        let reset_style = tuinix::TerminalStyle::RESET;
        let style = if self.selected { style.bold() } else { style };

        // Top border
        write!(frame, "{}", style).or_fail()?;
        write!(frame, "┌").or_fail()?;
        for _ in 1..width - 1 {
            write!(frame, "─").or_fail()?;
        }
        writeln!(frame, "┐").or_fail()?;

        // Middle rows with left/right borders
        for row in 1..height - 1 {
            write!(frame, "│").or_fail()?;
            if row == (height - 1) / 2 {
                let label = if shift {
                    self.key.shift_code.to_string()
                } else {
                    self.key.code.to_string()
                };
                let padding_left = (width - 2 - label.len()) / 2;
                let padding_right = width - 2 - padding_left - label.len();
                write!(
                    frame,
                    "{:padding_left$}{label}{:padding_right$}",
                    "",
                    "",
                    padding_left = padding_left,
                    padding_right = padding_right,
                )
                .or_fail()?;
            } else {
                write!(frame, "{:width$}", "", width = width - 2).or_fail()?;
            }
            writeln!(frame, "│").or_fail()?;
        }

        // Bottom border
        write!(frame, "└").or_fail()?;
        for _ in 1..width - 1 {
            write!(frame, "─").or_fail()?;
        }
        writeln!(frame, "┘").or_fail()?;
        write!(frame, "{}", reset_style).or_fail()?;

        Ok(frame)
    }
}
