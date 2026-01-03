use std::fmt::Write;
use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Layout {
    pub keys: Vec<Key>,
    pub preview: Option<Preview>,
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
        let mut preview = None;
        let mut next_newline_rows = 1;
        let mut default_size = tuinix::TerminalSize { rows: 3, cols: 3 };
        let mut position = tuinix::TerminalPosition::ZERO;
        let mut base_col = 0;
        for key_value in value.to_array()? {
            if let Some(blank_count) = key_value.to_member("blank")?.get() {
                let count: std::num::NonZeroUsize = blank_count.try_into()?;
                position.col += count.get();
                continue;
            }
            if let Some(newline_count) = key_value.to_member("newline")?.get() {
                let count: std::num::NonZeroUsize = newline_count.try_into()?;
                position.col = base_col;
                position.row += next_newline_rows - 1 + count.get();
                next_newline_rows = 1;
                continue;
            }
            if let Some(position_value) = key_value.to_member("base_position")?.get() {
                position.row = position_value.to_member("row")?.required()?.try_into()?;
                position.col = position_value.to_member("column")?.required()?.try_into()?;
                base_col = position.col;
                next_newline_rows = 1;
                continue;
            }
            if let Some(default_size_value) = key_value.to_member("default_size")?.get() {
                default_size = parse_size(default_size_value)?;
                continue;
            }
            if let Some(preview_value) = key_value.to_member("preview")?.get() {
                let columns = preview_value.to_member("columns")?.required()?.try_into()?;
                let size = tuinix::TerminalSize::rows_cols(1, columns);
                let region = tuinix::TerminalRegion { position, size };
                preview = Some(Preview {
                    region,
                    history: Vec::new(),
                });
                position = region.top_right();
                continue;
            }

            let key = Key::parse(key_value, position, default_size)?;

            position = key.region.top_right();
            position.col += 1;
            next_newline_rows = next_newline_rows.max(key.region.size.rows);

            keys.push(key);
        }
        Ok(Self { keys, preview })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SentKey {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

impl SentKey {
    fn is_visible(&self) -> bool {
        !(self.ctrl || self.alt || !self.code.is_char())
    }
}

#[derive(Debug)]
pub struct Preview {
    pub region: tuinix::TerminalRegion,
    history: Vec<SentKey>,
}

impl Preview {
    pub fn on_key_sent(&mut self, code: KeyCode, ctrl: bool, alt: bool) {
        let sent_key = SentKey { code, ctrl, alt };
        if sent_key.is_visible() {
            if self.history.last().is_some_and(|k| !k.is_visible()) {
                self.history.clear();
            }
            self.history.push(sent_key);
        } else {
            if self.history.last() != Some(&sent_key) {
                self.history.clear();
            }
            self.history.push(sent_key);
        }
    }

    pub fn to_frame(&self) -> orfail::Result<tuinix::TerminalFrame> {
        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.region.size);
        if self.history.is_empty() {
            return Ok(frame);
        }

        if let Some(k) = self.history.last()
            && !k.is_visible()
        {
            let style = tuinix::TerminalStyle::new().italic().bold();
            write!(frame, "> {style}").or_fail()?;

            if k.ctrl {
                write!(frame, "C-").or_fail()?
            };
            if k.alt {
                write!(frame, "M-").or_fail()?
            };
            write!(frame, "{}", k.code).or_fail()?;

            let repeat_count = self.history.len();
            if repeat_count > 1 {
                write!(frame, " (x{repeat_count})").or_fail()?;
            }
        } else {
            let style = tuinix::TerminalStyle::new().bold();
            write!(frame, "> {style}").or_fail()?;

            for k in &self.history {
                write!(frame, "{}", k.code).or_fail()?;
            }
            write!(frame, "{} ", tuinix::TerminalStyle::new().reverse()).or_fail()?;
        }

        let padding = " ".repeat(self.region.size.cols);
        let reset = tuinix::TerminalStyle::RESET;
        write!(frame, "{reset}{padding}").or_fail()?;

        Ok(frame)
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
        default_size: tuinix::TerminalSize,
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
            .unwrap_or(default_size);

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
}

impl KeyCode {
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Shift | Self::Ctrl | Self::Alt)
    }

    pub fn is_modifiable(self) -> bool {
        matches!(
            self,
            Self::Char(_) | Self::Up | Self::Down | Self::Left | Self::Right
        )
    }

    pub fn is_char(self) -> bool {
        matches!(self, Self::Char(_))
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
        }
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for KeyCode {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        match value.to_unquoted_string_str()?.as_ref() {
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
}

impl KeyState {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            press: KeyPressState::Neutral,
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
