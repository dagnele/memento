use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use glob::glob;
use zerocopy::IntoBytes;

use crate::config::WorkspaceConfig;
use crate::embedding::embed_text;
use crate::repository::workspace::WORKSPACE_DIR;
use crate::repository::workspace::{NewContentLayer, NewItem, NewVectorSpan, WorkspaceRepository};
use crate::text_file::read_text_file;
use crate::uri::{self, Namespace};

#[derive(Debug, Clone)]
pub struct IndexingReport {
    pub indexed_paths: Vec<String>,
    pub skipped_paths: Vec<String>,
    pub deleted_paths: Vec<String>,
}

pub fn index_paths(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    paths: &[String],
    force: bool,
) -> Result<IndexingReport> {
    if paths.is_empty() {
        bail!("at least one path is required");
    }

    let mut report = IndexingReport {
        indexed_paths: Vec::new(),
        skipped_paths: Vec::new(),
        deleted_paths: Vec::new(),
    };

    let expanded_paths = expand_input_paths(paths)?;

    for input_path in expanded_paths {
        let resolved_path = resolve_input_path(&input_path)?;

        if path_contains_workspace_dir(&resolved_path) {
            bail!(
                "cannot add paths inside `{}`: `{}`",
                WORKSPACE_DIR,
                input_path.replace('\\', "/")
            );
        }

        let discovered_files = discover_files(&resolved_path)?;

        for file_path in discovered_files {
            let normalized_path = normalized_workspace_path(&file_path)?;

            if !force
                && repository
                    .get_item_by_source_path(&normalized_path)?
                    .is_some()
            {
                report.skipped_paths.push(normalized_path);
                continue;
            }

            index_file(repository, config, &file_path, &normalized_path)?;
            report.indexed_paths.push(normalized_path);
        }
    }

    Ok(report)
}

fn expand_input_paths(paths: &[String]) -> Result<Vec<String>> {
    let mut expanded = Vec::new();

    for input_path in paths {
        if input_targets_workspace_dir(input_path) {
            bail!(
                "cannot add paths inside `{}`: `{}`",
                WORKSPACE_DIR,
                input_path.replace('\\', "/")
            );
        }

        if is_glob_pattern(input_path) {
            let matches = expand_glob_pattern(input_path)?;

            if matches.is_empty() {
                bail!("glob matched no paths: `{input_path}`");
            }

            expanded.extend(matches);
        } else {
            expanded.push(input_path.clone());
        }
    }

    Ok(expanded)
}

fn expand_glob_pattern(pattern: &str) -> Result<Vec<String>> {
    let matches = glob(pattern)
        .with_context(|| format!("invalid glob pattern: `{pattern}`"))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to expand glob pattern: `{pattern}`"))?;

    let mut normalized = matches
        .iter()
        .filter(|path| !path_contains_workspace_dir(path))
        .filter_map(|path| path.to_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    normalized.sort_unstable();
    normalized.dedup();

    Ok(normalized)
}

fn path_contains_workspace_dir(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == WORKSPACE_DIR)
}

fn input_targets_workspace_dir(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| component.as_os_str() == WORKSPACE_DIR)
}

fn is_glob_pattern(path: &str) -> bool {
    path.contains('*') || path.contains('?') || path.contains('[')
}

pub fn reindex_resource_paths(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    resource_paths: &[String],
) -> Result<IndexingReport> {
    let mut report = IndexingReport {
        indexed_paths: Vec::new(),
        skipped_paths: Vec::new(),
        deleted_paths: Vec::new(),
    };

    for resource_path in resource_paths {
        let absolute_path = std::env::current_dir()
            .context("failed to read current directory")?
            .join(resource_path);

        if !absolute_path.exists() {
            if let Some(item) = repository.get_item_by_source_path(resource_path)? {
                repository.delete_item(item.id)?;
                report.deleted_paths.push(resource_path.clone());
            }
            continue;
        }

        let resolved_path = absolute_path
            .canonicalize()
            .with_context(|| format!("failed to resolve path `{}`", absolute_path.display()))?;
        index_file(repository, config, &resolved_path, resource_path)?;
        report.indexed_paths.push(resource_path.clone());
    }

    Ok(report)
}

fn index_file(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    file_path: &Path,
    normalized_path: &str,
) -> Result<()> {
    index_item_file(
        repository,
        config,
        file_path,
        normalized_path,
        &uri::build_resource_uri(normalized_path),
        "resources",
        "resource_file",
    )
}

pub fn index_namespace_item(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    namespace: Namespace,
    uri_path: &str,
    workspace_source_path: &str,
) -> Result<String> {
    let file_path = Path::new(workspace_source_path);
    let kind = match namespace {
        Namespace::User => "user_item",
        Namespace::Agent => "agent_item",
        Namespace::Resources => bail!("resources items must be indexed through add or reindex"),
    };
    let uri = uri::build_namespace_item_uri(namespace, uri_path);

    index_item_file(
        repository,
        config,
        file_path,
        workspace_source_path,
        &uri,
        namespace.as_str(),
        kind,
    )?;

    Ok(uri)
}

fn index_item_file(
    repository: &WorkspaceRepository,
    config: &WorkspaceConfig,
    file_path: &Path,
    source_path: &str,
    uri: &str,
    namespace: &str,
    kind: &str,
) -> Result<()> {
    let metadata = fs::metadata(file_path)
        .with_context(|| format!("failed to read metadata for `{}`", file_path.display()))?;

    repository.upsert_item(&NewItem {
        uri,
        namespace,
        kind,
        source_path: Some(source_path),
        file_size_bytes: i64::try_from(metadata.len()).ok(),
        modified_at: system_time_to_unix_timestamp(metadata.modified().ok()).as_deref(),
    })?;

    let item = repository
        .get_item_by_source_path(source_path)?
        .ok_or_else(|| anyhow!("item missing after upsert: `{source_path}`"))?;

    repository.replace_content_layer(&NewContentLayer {
        item_id: item.id,
        layer: "detail",
        storage_kind: "disk",
        body: None,
        checksum: None,
    })?;

    let content = read_text_file(file_path)?;
    let segments = build_segments(
        &content,
        config.segment_line_count,
        config.segment_line_overlap,
    );
    let spans = segments
        .iter()
        .map(|segment| NewVectorSpan {
            item_id: item.id,
            layer: "detail",
            scope: "segment",
            start_line: Some(segment.start_line),
            end_line: Some(segment.end_line),
        })
        .collect::<Vec<_>>();

    repository.replace_item_spans(item.id, &spans)?;

    let stored_spans = repository.list_item_spans(item.id)?;

    if stored_spans.len() != segments.len() {
        bail!(
            "segment count mismatch for `{source_path}`: expected {}, got {}",
            segments.len(),
            stored_spans.len()
        );
    }

    for (segment, span) in segments.iter().zip(stored_spans.iter()) {
        let embedding = embed_text(&segment.text)?;

        repository.replace_vector_embedding(span.id, embedding.as_bytes())?;
    }

    Ok(())
}

fn resolve_input_path(path: &str) -> Result<PathBuf> {
    let candidate = PathBuf::from(path);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(candidate)
    };

    if !absolute.exists() {
        bail!("path does not exist: `{path}`");
    }

    absolute
        .canonicalize()
        .with_context(|| format!("failed to resolve path `{path}`"))
}

fn discover_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if !path.is_dir() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", path.display()))?;
        let entry_path = entry.path();

        if entry_path
            .file_name()
            .is_some_and(|name| name == ".memento")
        {
            continue;
        }

        if entry_path.is_dir() {
            files.extend(discover_files(&entry_path)?);
        } else if entry_path.is_file() {
            files.push(entry_path);
        }
    }

    files.sort();
    Ok(files)
}

fn normalized_workspace_path(path: &Path) -> Result<String> {
    let current_dir = std::env::current_dir()
        .context("failed to read current directory")?
        .canonicalize()
        .context("failed to resolve current directory")?;

    let relative = path
        .strip_prefix(&current_dir)
        .map(Path::to_path_buf)
        .map_err(|_| anyhow!("path `{}` is outside the current workspace", path.display()))?;

    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

#[derive(Debug, Clone)]
struct TextSegment {
    start_line: i64,
    end_line: i64,
    text: String,
}

fn build_segments(
    content: &str,
    segment_line_count: usize,
    segment_line_overlap: usize,
) -> Vec<TextSegment> {
    let lines = content.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        return vec![TextSegment {
            start_line: 1,
            end_line: 1,
            text: String::new(),
        }];
    }

    let step = segment_line_count
        .saturating_sub(segment_line_overlap)
        .max(1);
    let mut segments = Vec::new();
    let mut start_index = 0usize;

    while start_index < lines.len() {
        let end_index = (start_index + segment_line_count).min(lines.len());
        let chunk = &lines[start_index..end_index];

        segments.push(TextSegment {
            start_line: i64::try_from(start_index + 1).unwrap_or(i64::MAX),
            end_line: i64::try_from(end_index).unwrap_or(i64::MAX),
            text: chunk.join("\n"),
        });

        if end_index == lines.len() {
            break;
        }

        start_index += step;
    }

    segments
}

fn system_time_to_unix_timestamp(time: Option<SystemTime>) -> Option<String> {
    let duration = time?.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    Some(duration.as_secs().to_string())
}
