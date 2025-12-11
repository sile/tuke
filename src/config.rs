use std::path::Path;

use orfail::OrFail;

#[derive(Debug)]
pub struct Config {}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> orfail::Result<Self> {
        let text = std::fs::read(&path).or_fail_with(|e| {
            format!(
                "failed to load config file {}: {e}",
                path.as_ref().display()
            )
        })?;
        todo!()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {}
    }
}
