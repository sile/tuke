//! JSONC loading shared by the layout model.

use std::path::Path;

use crate::error::{Error, Result};

/// Reads `path` as JSONC and parses it into `T`.
pub fn load_file<P: AsRef<Path>, T>(path: P) -> Result<T>
where
    T: for<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError>,
{
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|error| {
        Error::message(format!("failed to read file '{}': {error}", path.display()))
    })?;
    load_str(&path.display().to_string(), &text)
}

/// Parses `text` as JSONC into `T`, using `name` in any error message.
pub fn load_str<T>(name: &str, text: &str) -> Result<T>
where
    T: for<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>, Error = nojson::JsonParseError>,
{
    let value = nojson::RawJson::parse_jsonc(text)
        .and_then(|(json, _)| T::try_from(json.value()))
        .map_err(|error| Error::json(name, text, error))?;
    Ok(value)
}
