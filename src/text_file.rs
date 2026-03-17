use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

pub fn read_text_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("failed to read `{}`", path.display()))?;

    if bytes.contains(&0) {
        bail!(
            "only text-based UTF-8 files are supported: `{}`",
            path.display()
        );
    }

    if std::str::from_utf8(&bytes).is_err() {
        bail!(
            "only text-based UTF-8 files are supported: `{}`",
            path.display()
        );
    }

    String::from_utf8(bytes)
        .with_context(|| format!("failed to decode `{}` as UTF-8", path.display()))
}
