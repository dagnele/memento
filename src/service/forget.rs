use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::uri::{Namespace, ParsedUri, parse_memento_uri};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgetResult {
    pub uri: String,
    pub path: String,
}

pub fn execute(uri: String) -> Result<ForgetResult> {
    let parsed = parse_memento_uri(&uri)?;
    match parsed {
        ParsedUri::Item {
            namespace: Namespace::User | Namespace::Agent,
            ..
        } => {}
        ParsedUri::Item {
            namespace: Namespace::Resources,
            ..
        } => bail!(
            "`forget` only supports `mem://user/...` and `mem://agent/...`; use `rm` for resources"
        ),
        ParsedUri::Root | ParsedUri::Namespace(_) => {
            bail!("`forget` requires a concrete user or agent item URI")
        }
    }

    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;

    let item = repository
        .get_item_by_uri(&uri)?
        .ok_or_else(|| anyhow::anyhow!("memory item not found: `{uri}`"))?;

    if item.namespace == "resources" {
        bail!("`forget` only supports Memento-owned user/agent items; use `rm` for resources")
    }

    let path = item
        .source_path
        .clone()
        .ok_or_else(|| anyhow::anyhow!("memory item has no backing path: `{uri}`"))?;

    if std::path::Path::new(&path).exists() {
        std::fs::remove_file(&path).with_context(|| format!("failed to delete `{path}`"))?;
    }

    repository.delete_item(item.id)?;

    Ok(ForgetResult {
        uri: item.uri,
        path,
    })
}
