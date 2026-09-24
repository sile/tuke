//! The software keyboard layout: JSON Lines parsing and the key model.

use std::path::Path;

use crate::error::Error;

/// Where the layout file asked the floating keyboard to sit.
///
/// The coordinates are the terminal's bottom-left corner as the origin: `col`
/// counts columns from the left, and `rows` counts rows up from the bottom to
/// the keyboard's bottom edge. They are kept as given rather than resolved
/// against a size, because the terminal can be resized: [`KeyboardPos::to_screen`]
/// resolves them again whenever the terminal size changes, so the keyboard
/// keeps its offset from the corner it was pinned to.
///
/// A layout that declares no `keyboard_pos` gets the default: the bottom-left
/// corner, [`KeyboardPos::ORIGIN`]. A `keyboard_pos` belongs to the layout it
/// is written in and does not carry over to the layouts after it, so each
/// layout names its own position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardPos {
    /// Columns from the terminal's left edge to the keyboard's left edge.
    pub col: usize,
    /// Rows from the terminal's bottom edge to the keyboard's bottom edge.
    pub rows: usize,
}

impl KeyboardPos {
    /// The origin of the coordinate system: the terminal's bottom-left corner.
    pub const ORIGIN: Self = Self { col: 0, rows: 0 };

    /// Resolves the position to the anchor the keyboard is laid out from, for
    /// a `terminal_size`-sized terminal.
    ///
    /// The anchor is the screen position of the keyboard's bottom-left corner:
    /// its last row and its leftmost column. `rows` counts up from the
    /// terminal's bottom row, so `rows` 0 puts the keyboard against the bottom
    /// edge. A `rows` past the top is clamped to the top row, so the anchor
    /// always names a row the terminal has.
    pub fn to_screen(self, terminal_size: tuinix::Size) -> tuinix::Position {
        tuinix::Position {
            row: terminal_size
                .rows
                .saturating_sub(1)
                .saturating_sub(self.rows),
            col: self.col,
        }
    }
}

impl Default for KeyboardPos {
    fn default() -> Self {
        Self::ORIGIN
    }
}

/// A software keyboard layout: the keys to draw.
#[derive(Debug)]
pub struct Layout {
    /// The keys, in the order the layout declares them.
    pub keys: Vec<Key>,
    /// Where the keyboard floats, from the layout's own `keyboard_pos` entry,
    /// or [`KeyboardPos::ORIGIN`] when it declares none.
    pub keyboard_pos: KeyboardPos,
}

/// The layouts a layout file defines, in the order they are declared.
///
/// A file is JSON Lines: one entry per line, where [`Layout`] entries lay out
/// keys and a `{"layout": NAME}` entry starts a new named layout. Entries
/// before the first `{"layout": …}` belong to a layout named `default`, so a
/// file that never names one is a single layout.
///
/// A `{"layout": NAME}` entry starts a layout from scratch: a board setting
/// such as `keyboard_pos`, `default_size`, `default_padding` or `base_position`
/// applies to the layout it is written in and is not inherited by the next
/// one. Reordering entries within a layout can still change what its keys
/// mean, because a key is placed at the cursor the entries before it leave;
/// but reordering whole layouts cannot, because no setting crosses a
/// `{"layout": …}` boundary.
///
/// A layout's keys carry a `switch_to` code that names another layout in the
/// same set, which is how one soft key swaps the keyboard for another. A name
/// no layout in the file declares is rejected when the file is read, at the
/// name that spelled it, so a switch that could never fire cannot get in.
#[derive(Debug)]
pub struct LayoutSet {
    layouts: Vec<NamedLayout>,
}

/// One named layout within a [`LayoutSet`].
#[derive(Debug)]
pub struct NamedLayout {
    /// The name a `switch_to` refers to it by.
    pub name: String,
    /// The keys to draw while it is showing.
    pub layout: Layout,
}

impl LayoutSet {
    /// Loads a layout set from a JSON Lines file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|error| {
            Error::message(format!("failed to read file '{}': {error}", path.display()))
        })?;
        Self::from_jsonl(&path.display().to_string(), &text)
    }

    /// Builds a layout set from the text of a JSON Lines file.
    ///
    /// `name` is used in any error message, so calling this on an embedded
    /// layout can name where the text came from. The entries are read in file
    /// order and a failure is reported at the line it came from.
    pub fn from_jsonl(name: &str, text: &str) -> crate::Result<Self> {
        let entries = crate::jsonl::parse_entries(name, text)?;
        let mut builder = LayoutSetBuilder::default();
        for entry in &entries {
            builder.accept(entry.json.value()).map_err(|error| {
                Error::json(
                    name,
                    line_text(text, Some(entry.line)),
                    Some(entry.line),
                    error,
                )
            })?;
        }
        builder
            .check_switches()
            .map_err(|error| Error::json(name, text, None, error))?;
        Ok(builder.finish())
    }

    /// The layouts in declaration order.
    pub fn layouts(&self) -> &[NamedLayout] {
        &self.layouts
    }

    /// Builds a layout set from named layouts, without validating their
    /// `switch_to` targets.
    ///
    /// It exists so tests and callers that build layouts in code can make a
    /// set without going through JSON Lines. [`LayoutSet::load_from_file`] is the
    /// way a user's file becomes a set, and it rejects a `switch_to` that
    /// names a layout the file does not define; a set built by this function
    /// has no such check, so a switch to a name it lacks is inert.
    pub fn from_named(layouts: Vec<NamedLayout>) -> Self {
        Self { layouts }
    }

    /// The layout named `name`, or `None` when the set has no such layout.
    pub fn get(&self, name: &str) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|named| named.name == name)
            .map(|named| &named.layout)
    }

    /// The first declared layout.
    ///
    /// It is what tuke shows when it starts, so a layout file needs no entry
    /// naming an "initial" layout: the first one it declares is the one the
    /// user sees.
    pub fn first(&self) -> &Layout {
        &self.layouts[0].layout
    }

    /// The name of the first declared layout.
    ///
    /// It is what [`LayoutSet::first`] would return, by name, so a caller that
    /// tracks the current layout can start from it.
    pub fn first_name(&self) -> &str {
        &self.layouts[0].name
    }
}

impl Default for LayoutSet {
    fn default() -> Self {
        Self::from_jsonl("default.jsonl", include_str!("../layouts/default.jsonl")).expect("bug")
    }
}

impl Layout {
    /// Loads a single layout from a JSON Lines file.
    ///
    /// This is the first layout of the file's [`LayoutSet`]: a file defining
    /// several layouts loads as its first one, the one it shows at startup.
    /// Prefer [`LayoutSet::load_from_file`] when the layouts switch.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        Ok(LayoutSet::load_from_file(path)?.into_first())
    }
}

impl Default for Layout {
    fn default() -> Self {
        LayoutSet::default().into_first()
    }
}

impl LayoutSet {
    /// Consumes the set and returns its first layout.
    fn into_first(self) -> Layout {
        self.layouts
            .into_iter()
            .next()
            .expect("a layout set always has at least one layout")
            .layout
    }
}

/// The text of the `line`-numbered line of `text`, for an error report.
///
/// A `None` `line` reports the whole file: the error is not tied to one line
/// (a `switch_to` target is only known once the file has been read to its end).
fn line_text(text: &str, line: Option<std::num::NonZeroUsize>) -> &str {
    match line {
        Some(line) => text.lines().nth(line.get() - 1).unwrap_or(text),
        None => text,
    }
}

/// Builds a [`LayoutSet`] from the entries of one layout file.
///
/// The entries are positional: each key is placed where the cursor sits and
/// moves the cursor past itself. The cursor lives in the [`LayoutBuilder`]
/// being filled rather than here, so a `{"layout": NAME}` entry ends the
/// current layout and starts the next one with a fresh cursor at the origin.
///
/// A `{"layout": NAME}` entry resets every board setting, not just the cursor:
/// a `keyboard_pos`, `default_size`, `default_padding` or `base_position`
/// belongs to the layout it is written in and does not carry over to the next
/// one. A layout that wants one says so in its own entries.
#[derive(Debug, Default)]
pub(crate) struct LayoutSetBuilder<'text, 'raw> {
    layouts: Vec<NamedLayout>,
    names: Vec<String>,
    current_name: Option<String>,
    current: LayoutBuilder,
    /// Every `switch_to` seen, paired with the value that named it.
    ///
    /// The target's existence cannot be checked while the entries are read:
    /// a switch can name a layout that is declared later in the same file, so
    /// the names are all known only once the file has been read to its end.
    /// The references are collected here and checked by
    /// [`LayoutSet::check_switches`] afterwards. The value is kept so an
    /// unknown target can be reported at the name that spelled it.
    switches: Vec<(String, nojson::RawJsonValue<'text, 'raw>)>,
}

impl<'text, 'raw> LayoutSetBuilder<'text, 'raw> {
    /// Applies one layout entry, either starting a new layout or adding to the
    /// current one.
    fn accept(
        &mut self,
        entry: nojson::RawJsonValue<'text, 'raw>,
    ) -> Result<(), nojson::JsonParseError> {
        let Some(name_value) = entry.to_member("layout")?.optional() else {
            self.record_switch(entry)?;
            return self.current.accept(entry);
        };

        let name = name_value.to_unquoted_string_str()?.to_string();
        if self.names.contains(&name) {
            return Err(name_value.invalid("duplicate layout name"));
        }
        self.names.push(name.clone());

        // The layout before the first `{"layout": …}` is the unnamed one, and
        // it is called `default`. When the file opens with a `{"layout": …}`
        // entry there is no such layout: the cursor has not moved and no key
        // has been placed, so the placeholder is dropped rather than pushing
        // an empty `default` in front of the named layouts.
        let previous_name = self.current_name.replace(name);
        let previous = std::mem::take(&mut self.current);
        // A `keyboard_pos` belongs to the layout it is written in, so the new
        // layout starts from the default corner rather than the one the
        // previous layout ended on. A layout that wants its own position says
        // so in its own entries. The cursor and the other settings reset the
        // same way, because they too describe a board rather than a file.
        if previous_name.is_some() || !previous.is_empty() {
            self.layouts.push(NamedLayout {
                name: previous_name.unwrap_or_else(|| "default".to_string()),
                layout: previous.finish(),
            });
        }
        Ok(())
    }

    /// Records the target of a `switch_to` entry, if `entry` has one.
    ///
    /// A `switch_to` key names its target under the same `key` member a send
    /// key takes its code from, so only entries whose `key` is an object can
    /// carry one. The value is kept along with the name so a missing target
    /// can be reported where the name was written.
    fn record_switch(
        &mut self,
        entry: nojson::RawJsonValue<'text, 'raw>,
    ) -> Result<(), nojson::JsonParseError> {
        let Some(key_value) = entry.to_member("key")?.optional() else {
            return Ok(());
        };
        if !key_value.kind().is_object() {
            return Ok(());
        }
        let Some(target) = key_value.to_member("switch_to")?.optional() else {
            return Ok(());
        };
        self.switches
            .push((target.to_unquoted_string_str()?.to_string(), target));
        Ok(())
    }

    /// Checks that every `switch_to` names a layout the file defines.
    ///
    /// It runs once the whole file has been read, so a switch may name a
    /// layout declared after it. A name the file never declares is an error
    /// reported at the `switch_to` value: a layout that silently does nothing
    /// when pressed is a typo the user cannot see.
    fn check_switches(&self) -> Result<(), nojson::JsonParseError> {
        let mut names: Vec<&str> = self.layouts.iter().map(|l| l.name.as_str()).collect();
        // The layout still being built is not in `layouts` yet, so its name
        // (or the `default` a file with no `{"layout": …}` entry takes) is
        // added by hand, matching what `finish` will push.
        if let Some(name) = &self.current_name {
            names.push(name);
        } else {
            names.push("default");
        }

        for (target, value) in &self.switches {
            if !names.contains(&target.as_str()) {
                return Err(value.invalid(format!("unknown layout '{target}'")));
            }
        }
        Ok(())
    }

    /// Finishes the set with the layout still being built.
    fn finish(mut self) -> LayoutSet {
        self.layouts.push(NamedLayout {
            name: self.current_name.unwrap_or_else(|| "default".to_string()),
            layout: self.current.finish(),
        });
        LayoutSet {
            layouts: self.layouts,
        }
    }
}

/// Builds one [`Layout`] from its entries, holding the parsing cursor.
#[derive(Debug)]
struct LayoutBuilder {
    keys: Vec<Key>,
    next_newline_rows: usize,
    default_size: tuinix::Size,
    default_padding: usize,
    position: tuinix::Position,
    base_col: usize,
    keyboard_pos: KeyboardPos,
}

impl Default for LayoutBuilder {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            next_newline_rows: 1,
            default_size: tuinix::Size { rows: 3, cols: 3 },
            default_padding: 1,
            position: tuinix::Position::ORIGIN,
            base_col: 0,
            keyboard_pos: KeyboardPos::ORIGIN,
        }
    }
}

impl LayoutBuilder {
    /// Handles the entries that move the cursor, returning whether `entry` was
    /// one of them.
    fn accept_cursor(
        &mut self,
        entry: nojson::RawJsonValue<'_, '_>,
    ) -> Result<bool, nojson::JsonParseError> {
        if let Some(blank_count) = entry.to_member("blank")?.optional() {
            let count: std::num::NonZeroUsize = blank_count.try_into()?;
            self.position.col += count.get();
            return Ok(true);
        }
        if let Some(newline_count) = entry.to_member("newline")?.optional() {
            let count: std::num::NonZeroUsize = newline_count.try_into()?;
            self.position.col = self.base_col;
            self.position.row += self.next_newline_rows - 1 + count.get();
            self.next_newline_rows = 1;
            return Ok(true);
        }
        if let Some(position_value) = entry.to_member("base_position")?.optional() {
            self.position.row = position_value.to_member("row")?.required()?.try_into()?;
            self.position.col = position_value.to_member("column")?.required()?.try_into()?;
            self.base_col = self.position.col;
            self.next_newline_rows = 1;
            return Ok(true);
        }
        if let Some(default_size_value) = entry.to_member("default_size")?.optional() {
            self.default_size = parse_size(default_size_value)?;
            return Ok(true);
        }
        if let Some(default_padding_value) = entry.to_member("default_padding")?.optional() {
            self.default_padding = default_padding_value.try_into()?;
            return Ok(true);
        }
        if let Some(keyboard_pos_value) = entry.to_member("keyboard_pos")?.optional() {
            let col = keyboard_pos_value
                .to_member("col")?
                .required()?
                .try_into()?;
            let rows = keyboard_pos_value
                .to_member("rows")?
                .required()?
                .try_into()?;
            self.keyboard_pos = KeyboardPos { col, rows };
            return Ok(true);
        }
        Ok(false)
    }

    /// Applies one entry: a cursor movement, or a key placed at the cursor.
    fn accept(
        &mut self,
        entry: nojson::RawJsonValue<'_, '_>,
    ) -> Result<(), nojson::JsonParseError> {
        if self.accept_cursor(entry)? {
            return Ok(());
        }

        let key = Key::parse(
            entry,
            self.position,
            self.default_size,
            self.default_padding,
        )?;
        let padding = key.padding;

        self.position = region_top_right(key.region);
        self.position.col += padding;
        self.next_newline_rows = self.next_newline_rows.max(key.region.size.rows);

        self.keys.push(key);
        Ok(())
    }

    /// Whether no entry has placed a key yet.
    ///
    /// A `{"layout": …}` entry that opens a file ends the placeholder layout
    /// before it, and an empty one is dropped: a layout with nothing in it is
    /// not something the user asked for.
    fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Finishes the layout being built.
    fn finish(self) -> Layout {
        Layout {
            keys: self.keys,
            keyboard_pos: self.keyboard_pos,
        }
    }
}

/// One soft key: what pressing it does, and where it sits in the layout.
#[derive(Debug, Clone)]
pub struct Key {
    /// What a press on the key does: send a code, or switch layouts.
    pub action: KeyAction,
    /// The key's rectangle, in layout coordinates.
    pub region: tuinix::Region,
    /// How many columns to leave before the next key on the row.
    ///
    /// A key is placed against the previous one plus this many columns, so a
    /// layout that writes `{"padding": 0}` puts the next key flush against
    /// this one and a wider value spreads the row out. It is separate from
    /// [`region`](Self::region) because the gap is not part of the key: a soft
    /// key is drawn inside its own rectangle, and the columns after it are
    /// only a hole in the row.
    ///
    /// The default comes from the layout's `default_padding`, so a compact
    /// board only has to say so once.
    pub padding: usize,
}

/// What pressing a soft key does.
///
/// A key types something (the common case), changes which layout the keyboard
/// shows, or types a whole string the user configured. All three are declared
/// under the same `key` member, so a layout spells them the same way and only
/// the key's value tells them apart: a string is a code to send, a
/// `{"switch_to": NAME}` object is a switch, and a `{"shortcut": …}` object is
/// a configured string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Send a key to the child when this key is pressed.
    Send {
        /// The code sent when the key is pressed without Shift.
        code: KeyCode,
        /// The code sent when the key is pressed with Shift active.
        shift_code: KeyCode,
    },
    /// Show another layout in the same set when this key is pressed.
    Switch {
        /// The name of the layout to show, as declared by `{"layout": NAME}`.
        to: String,
    },
    /// Type a configured string into the child when this key is pressed.
    ///
    /// The string is typed the way pressing its keys on the host keyboard
    /// would type it, so it carries no Enter: the user reads it back on the
    /// child's screen and decides what happens next. A key that ran a command
    /// outright could not be taken back, which is the wrong shape for a soft
    /// key, because a soft key is easy to hit by accident.
    ///
    /// It exists so a frequent command line can be one press rather than a
    /// dozen. The command is the user's, so tuke only carries the text.
    Shortcut {
        /// The label drawn on the key.
        ///
        /// The text itself is usually too long to draw (`attini approve` spans
        /// eleven columns), so the key shows this instead. It is required
        /// rather than derived from `text`, because the first word of two
        /// shortcuts can be the same and the labels would be indistinguishable.
        label: String,
        /// The text typed into the child, exactly as written.
        text: String,
    },
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        position: tuinix::Position,
        default_size: tuinix::Size,
        default_padding: usize,
    ) -> Result<Self, nojson::JsonParseError> {
        let key_value = value.to_member("key")?.required()?;
        let action = if key_value.kind().is_object() {
            if let Some(shortcut) = key_value.to_member("shortcut")?.optional() {
                // A shortcut types a string, which has no shifted form to
                // choose between, so a `shift` beside it names something that
                // cannot happen. Reporting it rather than ignoring it keeps a
                // layout from claiming a behaviour it does not have.
                if let Some(shift) = value.to_member("shift")?.optional() {
                    return Err(shift.invalid("a shortcut key cannot have a shift code"));
                }
                let label = shortcut
                    .to_member("label")?
                    .required()?
                    .to_unquoted_string_str()?
                    .to_string();
                let text = shortcut
                    .to_member("text")?
                    .required()?
                    .to_unquoted_string_str()?
                    .to_string();
                KeyAction::Shortcut { label, text }
            } else {
                let to = key_value
                    .to_member("switch_to")?
                    .required()?
                    .to_unquoted_string_str()?
                    .to_string();
                KeyAction::Switch { to }
            }
        } else {
            let code: KeyCode = key_value.try_into()?;
            let shift_code = if let Some(shift) = value.to_member("shift")?.optional() {
                shift.try_into()?
            } else {
                code.default_shift_code()
            };
            KeyAction::Send { code, shift_code }
        };

        let size = value
            .to_member("size")?
            .map(parse_size)?
            .unwrap_or(default_size);

        // Zero is the point of the member, so it is read as a plain count
        // rather than a `NonZeroUsize`.
        let padding = if let Some(padding_value) = value.to_member("padding")?.optional() {
            padding_value.try_into()?
        } else {
            default_padding
        };

        let region = tuinix::Region { position, size };

        Ok(Self {
            action,
            region,
            padding,
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
    /// Escape (called `Esc` in a layout).
    Escape,
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
            Self::Escape => termnix::KeyCode::Escape,
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
            Self::Escape => write!(f, "Esc"),
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
            "Esc" => Ok(Self::Escape),
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
