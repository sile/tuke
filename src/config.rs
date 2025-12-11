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
