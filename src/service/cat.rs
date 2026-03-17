use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::resource_state::{LiveResourceState, detect_live_state};
use crate::uri;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatResult {
    pub uri: String,
    pub content: String,
}

pub fn execute(uri: String) -> Result<CatResult> {
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;

    let parsed = uri::parse_memento_uri(&uri)?;
    let item = repository.get_item_by_uri(&uri)?;

    let Some(item) = item else {
        let entry_type = match parsed {
            uri::ParsedUri::Root | uri::ParsedUri::Namespace(_) => "namespace",
            uri::ParsedUri::Item { .. } => "directory",
        };
        bail!(
            "`{uri}` refers to a virtual {entry_type}; use `memento ls` or `memento show` instead"
        );
    };

    let live_state = detect_live_state(&item)?;
    let source_path = item
        .source_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("item has no readable source path: `{uri}`"))?;

    match live_state {
        LiveResourceState::Deleted => {
            bail!("`{uri}` is missing on disk; run `memento reindex {source_path}`")
        }
        LiveResourceState::Unreadable => {
            bail!("`{uri}` is unreadable on disk: `{source_path}`")
        }
        LiveResourceState::Ok | LiveResourceState::Modified => {}
    }

    let content = fs::read_to_string(Path::new(source_path))
        .with_context(|| format!("failed to read `{source_path}` for `{uri}`"))?;

    Ok(CatResult { uri, content })
}
