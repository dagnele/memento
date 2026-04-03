use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::WorkspaceConfig;
use crate::indexing::index_namespace_item;
use crate::repository::workspace::{AGENT_DIR, INDEX_FILE, USER_DIR, WorkspaceRepository};
use crate::service::shared::validate_workspace_embedding;
use crate::text_file::read_text_file;
use crate::uri::{Namespace, ParsedUri, parse_memento_uri};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RememberResult {
    pub uri: String,
    pub path: String,
}

pub fn execute(uri: String, file: Option<String>, text: Option<String>) -> Result<RememberResult> {
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;
    validate_workspace_embedding(&repository)?;
    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    repository.ensure_vector_schema_matches(config.embedding_dimension)?;

    let (namespace, relative_path) = parse_memory_item_uri(&uri)?;
    let remember_input = remember_input(file.as_deref(), text.as_deref())?;
    validate_destination_path(&relative_path, &remember_input)?;
    let workspace_path = workspace_file_path(namespace, &relative_path, &remember_input)?;
    let contents = remember_contents(&remember_input)?;

    if let Some(parent) = workspace_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }

    fs::write(&workspace_path, contents)
        .with_context(|| format!("failed to write `{}`", workspace_path.display()))?;

    let indexed_source_path = normalize_source_path(&workspace_path)?;
    let uri_path = namespace_uri_path(namespace, &workspace_path)?;
    let stored_uri = index_namespace_item(
        &repository,
        &config,
        namespace,
        &uri_path,
        &indexed_source_path,
    )?;

    Ok(RememberResult {
        uri: stored_uri,
        path: indexed_source_path,
    })
}

fn parse_memory_item_uri(uri: &str) -> Result<(Namespace, String)> {
    match parse_memento_uri(uri)? {
        ParsedUri::Item {
            namespace: Namespace::User,
            relative_path,
        } => Ok((Namespace::User, normalize_namespace_path(&relative_path)?)),
        ParsedUri::Item {
            namespace: Namespace::Agent,
            relative_path,
        } => Ok((Namespace::Agent, normalize_namespace_path(&relative_path)?)),
        ParsedUri::Item {
            namespace: Namespace::Resources,
            ..
        } => bail!("`remember` only supports `mem://user/...` and `mem://agent/...`"),
        ParsedUri::Root | ParsedUri::Namespace(_) => {
            bail!("`remember` requires a concrete user or agent item URI")
        }
    }
}

enum RememberInput<'a> {
    File(&'a str),
    Text(&'a str),
}

fn remember_input<'a>(file: Option<&'a str>, text: Option<&'a str>) -> Result<RememberInput<'a>> {
    match (file, text) {
        (Some(_), Some(_)) => bail!("provide either inline text or `--file`, but not both"),
        (None, None) => bail!("remember requires inline text or `--file <source>`"),
        (Some(file), None) => Ok(RememberInput::File(file)),
        (None, Some(text)) => Ok(RememberInput::Text(text)),
    }
}

fn remember_contents(input: &RememberInput<'_>) -> Result<String> {
    match input {
        RememberInput::File(path) => read_source_file(path),
        RememberInput::Text(text) => Ok((*text).to_string()),
    }
}

fn validate_destination_path(relative_path: &str, input: &RememberInput<'_>) -> Result<()> {
    if matches!(input, RememberInput::Text(_)) && !relative_path.ends_with(".md") {
        bail!(
            "inline text requires a Markdown destination URI ending in `.md`; use a URI like `mem://user/notes/todo.md`"
        );
    }

    Ok(())
}

fn read_source_file(path: &str) -> Result<String> {
    let candidate = PathBuf::from(path);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(candidate)
    };

    if !absolute.exists() {
        bail!("source file does not exist: `{path}`");
    }

    if !absolute.is_file() {
        bail!("source path is not a file: `{path}`");
    }

    read_text_file(&absolute)
}

fn normalize_namespace_path(path: &str) -> Result<String> {
    let candidate = Path::new(path);

    if candidate.is_absolute() {
        bail!("namespace path must be relative: `{path}`");
    }

    let mut parts = Vec::new();

    for component in candidate.components() {
        match component {
            Component::Normal(part) => {
                let segment = part.to_string_lossy().replace('\\', "/");

                if segment.is_empty() || segment == "." || segment == ".." {
                    bail!("invalid namespace path segment in `{path}`");
                }

                parts.push(segment);
            }
            Component::CurDir => {}
            _ => bail!("invalid namespace path: `{path}`"),
        }
    }

    if parts.is_empty() {
        bail!("namespace path must not be empty");
    }

    Ok(parts.join("/"))
}

fn workspace_file_path(
    namespace: Namespace,
    relative_path: &str,
    input: &RememberInput<'_>,
) -> Result<PathBuf> {
    let base = match namespace {
        Namespace::User => USER_DIR,
        Namespace::Agent => AGENT_DIR,
        Namespace::Resources => bail!("resources namespace is not valid for remember"),
    };

    let path_with_extension = if has_extension(relative_path) {
        relative_path.to_string()
    } else {
        match input {
            RememberInput::File(source) => {
                let source_path = Path::new(source);
                match source_path.extension().and_then(|ext| ext.to_str()) {
                    Some(extension) if !extension.is_empty() => {
                        format!("{relative_path}.{extension}")
                    }
                    _ => format!("{relative_path}.md"),
                }
            }
            RememberInput::Text(_) => unreachable!("inline text paths must end in .md"),
        }
    };

    Ok(PathBuf::from(base).join(path_with_extension))
}

fn normalize_source_path(path: &Path) -> Result<String> {
    Ok(path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

fn namespace_uri_path(namespace: Namespace, workspace_path: &Path) -> Result<String> {
    let base = match namespace {
        Namespace::User => USER_DIR,
        Namespace::Agent => AGENT_DIR,
        Namespace::Resources => bail!("resources namespace is not valid for remember"),
    };
    let relative = workspace_path
        .strip_prefix(base)
        .context("workspace path should start with namespace directory")?;
    let normalized = relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/");
    Ok(normalized)
}

fn has_extension(path: &str) -> bool {
    Path::new(path).extension().is_some()
}
