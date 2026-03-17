use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::uri::{ParsedUri, parse_memento_uri};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmResult {
    pub uri: String,
    pub path: String,
}

pub fn execute(target: String) -> Result<RmResult> {
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;

    let (lookup_path, uri) = if target.starts_with("mem://") {
        let parsed = parse_memento_uri(&target)?;
        match parsed {
            ParsedUri::Item {
                namespace: crate::uri::Namespace::Resources,
                relative_path,
            } => (relative_path, target.clone()),
            ParsedUri::Item { .. } => {
                bail!("`rm` only supports resource URIs; use `forget` for user/agent items")
            }
            ParsedUri::Root | ParsedUri::Namespace(_) => {
                bail!("`rm` requires a concrete resource item URI or path")
            }
        }
    } else {
        (target.clone(), crate::uri::build_resource_uri(&target))
    };

    let item = repository
        .get_item_by_source_path(&lookup_path)?
        .or_else(|| repository.get_item_by_uri(&uri).ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("resource is not tracked: `{target}`"))?;

    if item.namespace != "resources" {
        bail!("`rm` only supports tracked resources; use `forget` for Memento-owned items")
    }

    repository.delete_item(item.id)?;

    Ok(RmResult {
        uri: item.uri,
        path: item.source_path.unwrap_or(lookup_path),
    })
}
