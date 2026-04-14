use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::WorkspaceConfig;
use crate::indexing::index_paths;
use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::service::shared::validate_workspace_embedding;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddResult {
    pub indexed_paths: Vec<String>,
    pub metadata_only_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
}

pub fn execute(force: bool, paths: Vec<String>) -> Result<AddResult> {
    if paths.is_empty() {
        bail!("memento add requires at least one path");
    }

    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;
    validate_workspace_embedding(&repository)?;
    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    repository.ensure_vector_schema_matches(config.embedding_dimension)?;

    let report = index_paths(&repository, &config, &paths, force)?;

    Ok(AddResult {
        indexed_paths: report.indexed_paths,
        metadata_only_paths: report.metadata_only_paths,
        skipped_paths: report.skipped_paths,
    })
}
