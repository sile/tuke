use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Config {}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        crate::jsonc::load_file(path).or_fail()
    }
}

impl Default for Config {
    fn default() -> Self {
        crate::jsonc::load_str("default.json", include_str!("../configs/default.jsonc"))
            .expect("bug")
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Config {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        Ok(Self {})
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
                todo!()
            }
            _ => Err(ty.invalid("unknown action type")),
        }
    }
}

fn parse_key_code(
    value: nojson::RawJsonValue<'_, '_>,
) -> Result<tuinix::KeyCode, nojson::JsonParseError> {
    todo!()
}
