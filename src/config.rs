use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Config {
    pub keys: Vec<Key>,
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        crate::jsonc::load_file(path).or_fail()
    }
}

impl Default for Config {
    fn default() -> Self {
        match crate::jsonc::load_str("default.json", include_str!("../configs/default.jsonc")) {
            Ok(config) => config,
            Err(e) => panic!("[BUG] {e}"),
        }
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Config {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let mut keys = Vec::new();
        let mut last_key = None;
        for key_value in value.to_member("keys")?.required()?.to_array()? {
            let key = Key::parse(key_value, last_key.as_ref())?;
            last_key = Some(key.clone());
            keys.push(key);
        }
        Ok(Self { keys })
    }
}

#[derive(Debug, Clone)]
pub struct Key {
    pub code: KeyCode,
    pub region: tuinix::TerminalRegion,
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        last_key: Option<&Key>,
    ) -> Result<Self, nojson::JsonParseError> {
        let code = value.to_member("code")?.required()?.try_into()?;

        let size_member = value.to_member("size")?;
        let size = if let Some(last) = last_key {
            size_member.map(parse_size)?.unwrap_or(last.region.size)
        } else {
            size_member.required()?.map(parse_size)?
        };

        let position_member = value.to_member("position")?;
        let position = if let Some(last) = last_key {
            position_member.map(parse_position)?.unwrap_or_else(|| {
                let mut p = last.region.top_right();
                p.col += 1;
                p
            })
        } else {
            position_member.required()?.map(parse_position)?
        };

        let region = tuinix::TerminalRegion { position, size };

        Ok(Self { code, region })
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

    // Control Key Codes
    Quit,
    // FocusNextPane, FocusPrevPane
}

impl KeyCode {
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Shift | Self::Ctrl | Self::Alt)
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

            // Control
            Self::Quit => write!(f, "Quit"),
        }
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for KeyCode {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        match value.to_unquoted_string_str()?.as_ref() {
            "QUIT" => Ok(Self::Quit),
            "SHIFT" => Ok(Self::Shift),
            "CTRL" => Ok(Self::Ctrl),
            "ALT" => Ok(Self::Alt),
            "UP" => Ok(Self::Up),
            "DOWN" => Ok(Self::Down),
            "LEFT" => Ok(Self::Left),
            "RIGHT" => Ok(Self::Right),
            "ENTER" => Ok(Self::Enter),
            "BACKSPACE" => Ok(Self::Backspace),
            "DELETE" => Ok(Self::Delete),
            "TAB" => Ok(Self::Tab),
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
    if width < 8 {
        return Err(width_value.invalid("width must be at least 8"));
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

fn parse_position(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<tuinix::TerminalPosition, nojson::JsonParseError> {
    let x = value.to_member("x")?.required()?.try_into()?;
    let y = value.to_member("y")?.required()?.try_into()?;
    Ok(tuinix::TerminalPosition { row: y, col: x })
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

    pub fn to_frame(&self) -> orfail::Result<tuinix::TerminalFrame> {
        use std::fmt::Write;

        let mut frame: tuinix::TerminalFrame = tuinix::TerminalFrame::new(self.key.region.size);

        let width = self.key.region.size.cols;
        let height = self.key.region.size.rows;

        let style = tuinix::TerminalStyle::new();
        let style = match self.press {
            KeyPressState::Neutral => style,
            KeyPressState::Pressed => style.bold(),
            KeyPressState::Activated => style.reverse().bold(),
            KeyPressState::OneshotActivated => style.reverse(),
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
                let label = self.key.code.to_string();
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
