use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::WorkspaceConfig;
use crate::indexing::reindex_resource_paths;
use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::service::shared::validate_workspace_embedding;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexResult {
    pub indexed_paths: Vec<String>,
    pub deleted_paths: Vec<String>,
}

pub fn execute(paths: Vec<String>) -> Result<ReindexResult> {
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;
    validate_workspace_embedding(&repository)?;
    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    repository.ensure_vector_schema_matches(config.embedding_dimension)?;

    let resource_paths = if paths.is_empty() {
        repository
            .list_items()?
            .into_iter()
            .filter(|item| item.namespace == "resources")
            .filter_map(|item| item.source_path)
            .collect::<Vec<_>>()
    } else {
        paths
    };

    let report = reindex_resource_paths(&repository, &config, &resource_paths)?;

    Ok(ReindexResult {
        indexed_paths: report.indexed_paths,
        deleted_paths: report.deleted_paths,
    })
}
