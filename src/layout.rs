//! The software keyboard layout: JSONC parsing and the key/preview model.

use std::path::Path;

/// A software keyboard layout: the keys to draw and an optional send preview.
#[derive(Debug)]
pub struct Layout {
    /// The keys, in the order the layout declares them.
    pub keys: Vec<Key>,
    /// The send preview, when the layout declares one.
    pub preview: Option<Preview>,
}

impl Layout {
    /// Loads a layout from a JSONC file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        crate::jsonc::load_file(path)
    }
}

impl Default for Layout {
    fn default() -> Self {
        crate::jsonc::load_str("default.json", include_str!("../layouts/default.jsonc"))
            .expect("bug")
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Layout {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let mut keys = Vec::new();
        let mut preview = None;
        let mut next_newline_rows = 1;
        let mut default_size = tuinix::Size { rows: 3, cols: 3 };
        let mut position = tuinix::Position::ORIGIN;
        let mut base_col = 0;
        for key_value in value.to_array()? {
            if let Some(blank_count) = key_value.to_member("blank")?.optional() {
                let count: std::num::NonZeroUsize = blank_count.try_into()?;
                position.col += count.get();
                continue;
            }
            if let Some(newline_count) = key_value.to_member("newline")?.optional() {
                let count: std::num::NonZeroUsize = newline_count.try_into()?;
                position.col = base_col;
                position.row += next_newline_rows - 1 + count.get();
                next_newline_rows = 1;
                continue;
            }
            if let Some(position_value) = key_value.to_member("base_position")?.optional() {
                position.row = position_value.to_member("row")?.required()?.try_into()?;
                position.col = position_value.to_member("column")?.required()?.try_into()?;
                base_col = position.col;
                next_newline_rows = 1;
                continue;
            }
            if let Some(default_size_value) = key_value.to_member("default_size")?.optional() {
                default_size = parse_size(default_size_value)?;
                continue;
            }
            if let Some(preview_value) = key_value.to_member("preview")?.optional() {
                let width = preview_value.to_member("width")?.required()?.try_into()?;
                let size = tuinix::Size {
                    rows: 1,
                    cols: width,
                };
                let region = tuinix::Region { position, size };
                preview = Some(Preview {
                    region,
                    history: Vec::new(),
                });
                position = region_top_right(region);
                continue;
            }

            let key = Key::parse(key_value, position, default_size)?;

            position = region_top_right(key.region);
            position.col += 1;
            next_newline_rows = next_newline_rows.max(key.region.size.rows);

            keys.push(key);
        }
        Ok(Self { keys, preview })
    }
}

/// One key the user sent, as recorded by the preview.
#[derive(Debug, PartialEq, Eq)]
struct SentKey {
    /// The layout key code that was sent.
    code: KeyCode,
    /// Whether Ctrl was applied.
    ctrl: bool,
    /// Whether Alt was applied.
    alt: bool,
}

impl SentKey {
    fn is_visible(&self) -> bool {
        !(self.ctrl || self.alt || !self.code.is_char())
    }
}

/// The send preview: a row that shows the keys most recently sent.
#[derive(Debug)]
pub struct Preview {
    /// Where the preview is drawn, in layout coordinates.
    pub region: tuinix::Region,
    /// The keys sent so far, in order; cleared when the display would change
    /// shape (a visible run followed by an invisible one, or vice versa).
    history: Vec<SentKey>,
}

impl Preview {
    /// Records a key sent to the child, updating the preview's history.
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

    /// Renders the preview into a frame the size of its region.
    pub fn to_frame(&self) -> tuinix::Frame {
        let mut frame = tuinix::Frame::new(self.region.size);
        let mut at = put_text(
            &mut frame,
            tuinix::Position::ORIGIN,
            "> ",
            tuinix::Style::new(),
        );

        if let Some(k) = self.history.last()
            && !k.is_visible()
        {
            let style = tuinix::Style::new().italic().bold();

            let mut label = String::new();
            if k.ctrl {
                label.push_str("C-");
            }
            if k.alt {
                label.push_str("M-");
            }
            label.push_str(&k.code.to_string());

            let repeat_count = self.history.len();
            if repeat_count > 1 {
                label.push_str(&format!(" (x{repeat_count})"));
            }

            at = put_text(&mut frame, at, &label, style);
        } else if !self.history.is_empty() {
            let style = tuinix::Style::new().bold();
            for k in &self.history {
                at = put_text(&mut frame, at, &k.code.to_string(), style);
            }
            at = put_text(&mut frame, at, " ", style.reverse());
        }

        // Fill the rest of the row with blanks so the counter sits at the right
        // edge, marked by the trailing `>`.
        let padding = self.region.size.cols.saturating_sub(at.col + 1);
        put_text(
            &mut frame,
            at,
            &" ".repeat(padding + 1),
            tuinix::Style::RESET,
        );
        put_text(
            &mut frame,
            tuinix::Position {
                row: 0,
                col: self.region.size.cols.saturating_sub(1),
            },
            ">",
            tuinix::Style::RESET,
        );

        frame
    }
}

/// One soft key: its label codes and where it sits in the layout.
#[derive(Debug, Clone)]
pub struct Key {
    /// The code sent when the key is pressed without Shift.
    pub code: KeyCode,
    /// The code sent when the key is pressed with Shift active.
    pub shift_code: KeyCode,
    /// The key's rectangle, in layout coordinates.
    pub region: tuinix::Region,
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        position: tuinix::Position,
        default_size: tuinix::Size,
    ) -> Result<Self, nojson::JsonParseError> {
        let code: KeyCode = value.to_member("key")?.required()?.try_into()?;

        let shift_code = if let Some(shift) = value.to_member("shift")?.optional() {
            shift.try_into()?
        } else {
            code.default_shift_code()
        };

        let size = value
            .to_member("size")?
            .map(parse_size)?
            .unwrap_or(default_size);

        let region = tuinix::Region { position, size };

        Ok(Self {
            code,
            shift_code,
            region,
        })
    }
}

/// A layout key code, in tmux-compatible notation.
///
/// The textual form ([`Display`](std::fmt::Display)) is what a layout
/// declares: `BSpace`, `BTab`, `C-`, `M-`, `S-`, the arrow names, or a single
/// printable character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCode {
    /// A single printable character.
    Char(char),
    /// The Shift modifier, held or one-shot.
    Shift,
    /// The Ctrl modifier, held or one-shot.
    Ctrl,
    /// The Alt modifier (called `M-` in a layout).
    Alt,
    /// The Up arrow.
    Up,
    /// The Down arrow.
    Down,
    /// The Left arrow.
    Left,
    /// The Right arrow.
    Right,
    /// Enter.
    Enter,
    /// Backspace (called `BSpace` in a layout).
    Backspace,
    /// Delete.
    Delete,
    /// Tab.
    Tab,
    /// Shift+Tab (called `BTab` in a layout).
    BackTab,
}

impl KeyCode {
    /// Whether this is a modifier key (`Shift`, `Ctrl`, or `Alt`), which holds
    /// state rather than being sent on its own.
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Shift | Self::Ctrl | Self::Alt)
    }

    /// Whether this key can carry Ctrl or Alt when it is sent.
    pub fn is_modifiable(self) -> bool {
        matches!(
            self,
            Self::Char(_) | Self::Up | Self::Down | Self::Left | Self::Right
        )
    }

    /// Whether this is a single-character key.
    pub fn is_char(self) -> bool {
        matches!(self, Self::Char(_))
    }

    /// The code sent for this key when Shift is active and the layout names
    /// no explicit `shift` code.
    pub fn default_shift_code(self) -> Self {
        match self {
            Self::Char(c) => Self::Char(c.to_ascii_uppercase()),
            Self::Tab => Self::BackTab,
            other => other,
        }
    }

    /// Maps this layout key code to the terminal key code to send to the
    /// child's PTY.
    ///
    /// The modifier codes ([`KeyCode::Shift`], [`KeyCode::Ctrl`],
    /// [`KeyCode::Alt`]) are keyboard state, not keys sent on their own, so
    /// they have no `termnix` key code and map to `None`. The caller carries
    /// their effect in [`termnix::Modifiers`] instead.
    pub fn to_termnix(self) -> Option<termnix::KeyCode> {
        Some(match self {
            Self::Char(c) => termnix::KeyCode::Char(c),
            Self::Up => termnix::KeyCode::Up,
            Self::Down => termnix::KeyCode::Down,
            Self::Left => termnix::KeyCode::Left,
            Self::Right => termnix::KeyCode::Right,
            Self::Enter => termnix::KeyCode::Enter,
            Self::Backspace => termnix::KeyCode::Backspace,
            Self::Delete => termnix::KeyCode::Delete,
            Self::Tab => termnix::KeyCode::Tab,
            Self::BackTab => termnix::KeyCode::Tab,
            Self::Shift | Self::Ctrl | Self::Alt => return None,
        })
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
            Self::BackTab => write!(f, "BTab"),
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
            "BTab" => Ok(Self::BackTab),
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

fn parse_size(value: nojson::RawJsonValue<'_, '_>) -> Result<tuinix::Size, nojson::JsonParseError> {
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

    Ok(tuinix::Size {
        rows: height,
        cols: width,
    })
}

/// How a soft key is currently held, which drives its highlight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPressState {
    /// Idle: no modifier is pending for this key and it is not highlighted.
    Neutral,
    /// A modifier is held down (armed), so the key highlights as active.
    Activated,
    /// A modifier was tapped once and will apply to the next key only.
    OneshotActivated,
    /// A normal (non-modifier) key that was just pressed.
    Pressed,
}

/// A soft key together with its current press state.
#[derive(Debug, Clone)]
pub struct KeyState {
    /// The key's static description from the layout.
    pub key: Key,
    /// How the key is currently held.
    pub press: KeyPressState,
}

impl KeyState {
    /// Creates a key state in the [`KeyPressState::Neutral`] state.
    pub fn new(key: Key) -> Self {
        Self {
            key,
            press: KeyPressState::Neutral,
        }
    }
}

fn region_top_right(region: tuinix::Region) -> tuinix::Position {
    tuinix::Position {
        row: region.position.row,
        col: region.position.col + region.size.cols,
    }
}

/// Writes `text` into `frame` starting at `at`, advancing one column per
/// character, and returns the position just past the last written character.
///
/// A newline moves to the start of the next row, mirroring the `write!`-based
/// drawing this replaced.
fn put_text(
    frame: &mut tuinix::Frame,
    at: tuinix::Position,
    text: &str,
    style: tuinix::Style,
) -> tuinix::Position {
    let mut at = at;
    for c in text.chars() {
        if c == '\n' {
            at = at.next_line();
            continue;
        }
        let ch = tuinix::Char::new(c, 1, style).expect("not a control character");
        at = frame.put_char(at, ch);
    }
    at
}
