use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Config {}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        let text = std::fs::read_to_string(&path).or_fail_with(|e| {
            format!(
                "failed to read config file {}: {e}",
                path.as_ref().display()
            )
        })?;
        let config_json = nojson::RawJson::parse_jsonc(&text)
            .or_fail_with(|e| {
                format!(
                    "failed to parse config file {}: {e}",
                    path.as_ref().display()
                )
            })?
            .0;
        Self::try_from(config_json.value()).or_fail()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {}
    }
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for Config {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}
