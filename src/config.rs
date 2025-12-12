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
    pub action: Action,
}

impl Key {
    fn parse(
        value: nojson::RawJsonValue<'_, '_>,
        _last_key: Option<&Key>,
    ) -> Result<Self, nojson::JsonParseError> {
        let action = value.to_member("action")?.required()?.try_into()?;
        Ok(Self { action })
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    SendKey { code: tuinix::KeyCode },
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Action {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        let ty = value.to_member("type")?.required()?;
        match ty.to_unquoted_string_str()?.as_ref() {
            "send-key" => {
                let code = parse_key_code(value.to_member("code")?.required()?)?;
                Ok(Self::SendKey { code })
            }
            _ => Err(ty.invalid("unknown action type")),
        }
    }
}

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
