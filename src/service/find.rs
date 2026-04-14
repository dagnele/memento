use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use zerocopy::IntoBytes;

use crate::embedding::{current_embedding_profile, embed_text};
use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::resource_state::detect_live_state_from_source;
use crate::timing::{log_timing, timing_enabled};

/// Maximum L2 distance for a match to be considered relevant.
/// Results beyond this threshold are filtered out so that unrelated items
/// do not appear in search results. For normalized embedding vectors the L2
/// distance ranges from 0 (identical) to 2 (opposite); a threshold of 1.2
/// keeps meaningfully similar results while discarding near-orthogonal noise.
const MAX_MATCH_DISTANCE: f64 = 1.2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindMatch {
    pub uri: String,
    pub distance: f64,
    pub layer: String,
    pub scope: String,
    pub locator: String,
    pub kind: String,
    pub namespace: String,
    pub source_path: Option<String>,
    pub preview: Option<String>,
    pub live_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindResult {
    pub query: String,
    pub matches: Vec<FindMatch>,
}

pub fn execute(query: String) -> Result<FindResult> {
    let total_start = Instant::now();
    let step_start = Instant::now();
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;
    validate_workspace_embedding(&repository)?;
    let config = crate::config::WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    repository.ensure_vector_schema_matches(config.embedding_dimension)?;
    log_timing("find_setup", step_start.elapsed());

    let embed_start = Instant::now();
    let query_embedding = embed_text(&query)?;
    log_timing("find_embed_query", embed_start.elapsed());

    let search_start = Instant::now();
    let results = repository.search_similar_items(query_embedding.as_bytes(), 10)?;
    log_timing("find_search_vectors", search_start.elapsed());

    let result_count = results.len();
    let render_start = Instant::now();
    let matches = results
        .into_iter()
        .filter(|result| result.distance <= MAX_MATCH_DISTANCE)
        .map(|result| {
            let preview = read_preview(
                &repository,
                result.source_path.as_deref(),
                result.layer.as_str(),
                result.start_line,
                result.end_line,
                result.uri.as_str(),
            )?;

            Ok(FindMatch {
                uri: result.uri,
                distance: result.distance,
                layer: result.layer,
                scope: result.scope,
                locator: format_locator(result.start_line, result.end_line),
                kind: result.kind,
                namespace: result.namespace,
                preview,
                live_state: detect_live_state_from_source(
                    result.source_path.as_deref(),
                    result.file_size_bytes,
                    result.modified_at.as_deref(),
                )
                .ok()
                .map(|state| state.as_str().to_string()),
                source_path: result.source_path,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    log_timing("find_render_results", render_start.elapsed());
    log_timing("find_total", total_start.elapsed());

    if timing_enabled() {
        eprintln!("[memento:timing] result_count={result_count}");
    }

    Ok(FindResult { query, matches })
}

fn validate_workspace_embedding(repository: &WorkspaceRepository) -> Result<()> {
    let profile = current_embedding_profile()?;
    let stored_model = repository.get_workspace_meta("embedding_model")?;
    let stored_dimension = repository.get_workspace_meta("embedding_dimension")?;

    if stored_model.as_deref() != Some(profile.name)
        || stored_dimension.as_deref() != Some(&profile.dimension.to_string())
    {
        anyhow::bail!(
            "workspace embedding model does not match config; rerun `memento init --model {}` or fix `.memento/config.toml`",
            profile.name
        );
    }

    Ok(())
}

fn format_locator(start_line: Option<i64>, end_line: Option<i64>) -> String {
    match (start_line, end_line) {
        (Some(start), Some(end)) => format!("lines={start}-{end}"),
        (Some(start), None) => format!("start_line={start}"),
        (None, Some(end)) => format!("end_line={end}"),
        (None, None) => "lines=-".to_string(),
    }
}

fn read_preview(
    repository: &WorkspaceRepository,
    path: Option<&str>,
    layer: &str,
    start_line: Option<i64>,
    end_line: Option<i64>,
    uri: &str,
) -> Result<Option<String>> {
    if let Some(preview) = read_stored_preview(repository, uri, layer, start_line, end_line)? {
        return Ok(Some(preview));
    }

    let Some(path) = path else {
        return Ok(None);
    };

    let path = Path::new(path);
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(_) => return Ok(None),
    };

    let start = start_line.unwrap_or(1).max(1) as usize;
    let end = end_line
        .unwrap_or(start_line.unwrap_or(1))
        .max(start as i64) as usize;
    let lines = contents.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        return Ok(None);
    }

    let start_index = start.saturating_sub(1).min(lines.len());
    let end_index = end.min(lines.len());

    if start_index >= end_index {
        return Ok(None);
    }

    let preview = lines[start_index..end_index]
        .iter()
        .take(4)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");

    if preview.is_empty() {
        return Ok(None);
    }

    Ok(Some(preview))
}

fn read_stored_preview(
    repository: &WorkspaceRepository,
    uri: &str,
    layer: &str,
    start_line: Option<i64>,
    end_line: Option<i64>,
) -> Result<Option<String>> {
    let Some(item) = repository.get_item_by_uri(uri)? else {
        return Ok(None);
    };

    let Some(content_layer) = repository.get_content_layer(item.id, layer)? else {
        return Ok(None);
    };

    let Some(body) = content_layer.body else {
        return Ok(None);
    };

    let lines = body.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        return Ok(None);
    }

    let start = start_line.unwrap_or(1).max(1) as usize;
    let end = end_line
        .unwrap_or(start_line.unwrap_or(1))
        .max(start as i64) as usize;
    let start_index = start.saturating_sub(1).min(lines.len());
    let end_index = end.min(lines.len());

    if start_index >= end_index {
        return Ok(None);
    }

    let preview = lines[start_index..end_index]
        .iter()
        .take(4)
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");

    if preview.is_empty() {
        return Ok(None);
    }

    Ok(Some(preview))
}
