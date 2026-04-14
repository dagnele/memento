use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::config::WorkspaceConfig;
use crate::indexing::{index_namespace_item, reindex_resource_paths};
use crate::repository::workspace::{AGENT_DIR, INDEX_FILE, USER_DIR, WorkspaceRepository};
use crate::service::shared::validate_workspace_embedding;
use crate::uri::Namespace;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReindexResult {
    pub indexed_paths: Vec<String>,
    pub metadata_only_paths: Vec<String>,
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
            .filter_map(|item| item.source_path)
            .collect::<Vec<_>>()
    } else {
        paths
            .into_iter()
            .map(|path| normalize_input_path(&path))
            .collect::<Result<Vec<_>>>()?
    };

    let mut indexed_paths = Vec::new();
    let mut metadata_only_paths = Vec::new();
    let mut deleted_paths = Vec::new();

    for path in resource_paths {
        match reindex_path(&repository, &config, &path)? {
            ReindexPathOutcome::Indexed => indexed_paths.push(path),
            ReindexPathOutcome::MetadataOnly => metadata_only_paths.push(path),
            ReindexPathOutcome::Deleted => deleted_paths.push(path),
            ReindexPathOutcome::Skipped => {}
        }
    }

    Ok(ReindexResult {
        indexed_paths,
        metadata_only_paths,
        deleted_paths,
    })
}

enum ReindexPathOutcome {
    Indexed,
    MetadataOnly,
    Deleted,
    Skipped,
}

fn reindex_path(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    source_path: &str,
) -> Result<ReindexPathOutcome> {
    if let Some(item) = repository.get_item_by_source_path(source_path)? {
        if !Path::new(source_path).exists() {
            repository.delete_item(item.id)?;
            return Ok(ReindexPathOutcome::Deleted);
        }

        return match item.namespace.as_str() {
            "resources" => reindex_resource_file(repository, config, source_path),
            "user" => reindex_namespace_file(repository, config, Namespace::User, source_path),
            "agent" => reindex_namespace_file(repository, config, Namespace::Agent, source_path),
            namespace => bail!("unsupported indexed namespace for `{source_path}`: `{namespace}`"),
        };
    }

    if !Path::new(source_path).exists() {
        return Ok(ReindexPathOutcome::Skipped);
    }

    if let Some(namespace) = namespace_from_source_path(source_path) {
        return reindex_namespace_file(repository, config, namespace, source_path);
    }

    reindex_resource_file(repository, config, source_path)
}

fn reindex_resource_file(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    source_path: &str,
) -> Result<ReindexPathOutcome> {
    let report = reindex_resource_paths(repository, config, &[source_path.to_string()])?;

    if !report.indexed_paths.is_empty() {
        Ok(ReindexPathOutcome::Indexed)
    } else if !report.metadata_only_paths.is_empty() {
        Ok(ReindexPathOutcome::MetadataOnly)
    } else if !report.deleted_paths.is_empty() {
        Ok(ReindexPathOutcome::Deleted)
    } else {
        Ok(ReindexPathOutcome::Skipped)
    }
}

fn reindex_namespace_file(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    namespace: Namespace,
    source_path: &str,
) -> Result<ReindexPathOutcome> {
    let uri_path = namespace_uri_path(namespace, source_path)?;
    index_namespace_item(repository, config, namespace, &uri_path, source_path)?;
    Ok(ReindexPathOutcome::Indexed)
}

fn normalize_input_path(path: &str) -> Result<String> {
    let candidate = PathBuf::from(path);
    let relative = if candidate.is_absolute() {
        let current_dir = std::env::current_dir()
            .context("failed to read current directory")?
            .canonicalize()
            .context("failed to resolve current directory")?;
        candidate
            .strip_prefix(&current_dir)
            .map(Path::to_path_buf)
            .map_err(|_| anyhow!("path `{path}` is outside the current workspace"))?
    } else {
        candidate
    };

    let mut parts = Vec::new();

    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().replace('\\', "/")),
            Component::CurDir => {}
            _ => bail!("invalid reindex path: `{path}`"),
        }
    }

    if parts.is_empty() {
        bail!("reindex path must not be empty")
    }

    Ok(parts.join("/"))
}

fn namespace_from_source_path(source_path: &str) -> Option<Namespace> {
    if source_path == USER_DIR || source_path.starts_with(&format!("{USER_DIR}/")) {
        Some(Namespace::User)
    } else if source_path == AGENT_DIR || source_path.starts_with(&format!("{AGENT_DIR}/")) {
        Some(Namespace::Agent)
    } else {
        None
    }
}

fn namespace_uri_path(namespace: Namespace, source_path: &str) -> Result<String> {
    let base = match namespace {
        Namespace::User => USER_DIR,
        Namespace::Agent => AGENT_DIR,
        Namespace::Resources => bail!("resources namespace is not valid here"),
    };

    let relative = Path::new(source_path)
        .strip_prefix(base)
        .with_context(|| format!("path `{source_path}` is not inside `{base}`"))?;

    let normalized = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/");

    if normalized.is_empty() {
        bail!("namespace file path must not be empty: `{source_path}`")
    }

    Ok(normalized)
}
