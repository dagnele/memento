use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

pub fn read_text_file(path: &Path) -> Result<String> {
    match try_read_text_file(path)? {
        Some(content) => Ok(content),
        None => bail!(
            "only text-based UTF-8 files are supported: `{}`",
            path.display()
        ),
    }
}

pub fn try_read_text_file(path: &Path) -> Result<Option<String>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read `{}`", path.display()))?;

    if bytes.contains(&0) {
        return Ok(None);
    }

    if std::str::from_utf8(&bytes).is_err() {
        return Ok(None);
    }

    String::from_utf8(bytes)
        .map(Some)
        .with_context(|| format!("failed to decode `{}` as UTF-8", path.display()))
}
