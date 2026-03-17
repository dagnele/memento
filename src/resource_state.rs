use std::fs;
use std::path::Path;
use std::time::SystemTime;

use anyhow::Result;

use crate::repository::workspace::ItemRecord;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum LiveResourceState {
    Ok,
    Modified,
    Deleted,
    Unreadable,
}

impl LiveResourceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Unreadable => "unreadable",
        }
    }
}

pub fn detect_live_state(item: &ItemRecord) -> Result<LiveResourceState> {
    detect_live_state_from_source(
        item.source_path.as_deref(),
        item.file_size_bytes,
        item.modified_at.as_deref(),
    )
}

pub fn detect_live_state_from_source(
    source_path: Option<&str>,
    file_size_bytes: Option<i64>,
    modified_at: Option<&str>,
) -> Result<LiveResourceState> {
    let Some(source_path) = source_path else {
        return Ok(LiveResourceState::Ok);
    };

    let path = Path::new(source_path);

    if !path.exists() {
        return Ok(LiveResourceState::Deleted);
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(LiveResourceState::Unreadable),
    };

    let current_size = i64::try_from(metadata.len()).ok();
    let current_modified = system_time_to_unix_timestamp(metadata.modified().ok());

    if file_size_bytes != current_size || modified_at != current_modified.as_deref() {
        return Ok(LiveResourceState::Modified);
    }

    Ok(LiveResourceState::Ok)
}

fn system_time_to_unix_timestamp(time: Option<SystemTime>) -> Option<String> {
    let duration = time?.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(duration.as_secs().to_string())
}
