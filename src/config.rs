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
    pub label: String,
    pub action: Action,
    pub region: tuinix::TerminalRegion,
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        last_key: Option<&Key>,
    ) -> Result<Self, nojson::JsonParseError> {
        let label = value.to_member("label")?.required()?.try_into()?;
        let action = value.to_member("action")?.required()?.try_into()?;

        let size_member = value.to_member("size")?;
        let size = if let Some(last) = last_key {
            size_member.map(parse_size)?.unwrap_or(last.region.size)
        } else {
            size_member.required()?.map(parse_size)?
        };

        let position_member = value.to_member("position")?;
        let position = if let Some(last) = last_key {
            position_member
                .map(parse_position)?
                .unwrap_or(last.region.top_right())
        } else {
            position_member.required()?.map(parse_position)?
        };

        let region = tuinix::TerminalRegion { position, size };

        Ok(Self {
            label,
            action,
            region,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    SendLabel,
    // SendKey { code: tuinix::KeyCode },
    // Command { name:PathBuf, args:Vec<String>}
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Action {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let ty = value.to_member("type")?.required()?;
        match ty.to_unquoted_string_str()?.as_ref() {
            "send-label" => Ok(Self::SendLabel),
            _ => Err(ty.invalid("unknown action type")),
        }
    }
}

/*
fn parse_key_code(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<tuinix::KeyCode, nojson::JsonParseError> {
    let code = value.to_unquoted_string_str()?;
    if code.len() == 1
        && let Some(c) = code.chars().next()
        && c.is_ascii()
        && !c.is_ascii_control()
    {
        Ok(tuinix::KeyCode::Char(c))
    } else {
        Err(value.invalid("unknown key code"))
    }
}
*/

fn parse_size(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<tuinix::TerminalSize, nojson::JsonParseError> {
    let width = value.to_member("width")?.required()?.try_into()?;
    let height = value.to_member("height")?.required()?.try_into()?;
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
