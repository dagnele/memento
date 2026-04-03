use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{AGENT_DIR, INDEX_FILE, USER_DIR, WorkspaceRepository};
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

    let item = match repository.get_item_by_uri(&uri)? {
        Some(item) => item,
        None => {
            if let Some(result) = forget_directory(&parsed, &uri)? {
                return Ok(result);
            }

            return Err(anyhow::anyhow!("memory item not found: `{uri}`"));
        }
    };

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

fn forget_directory(parsed: &ParsedUri, uri: &str) -> Result<Option<ForgetResult>> {
    let ParsedUri::Item {
        namespace: Namespace::User | Namespace::Agent,
        relative_path,
    } = parsed
    else {
        return Ok(None);
    };

    let namespace = match parsed {
        ParsedUri::Item {
            namespace,
            relative_path: _,
        } => *namespace,
        _ => unreachable!(),
    };

    let path = namespace_dir_path(namespace, relative_path);

    if !path.exists() {
        return Ok(None);
    }

    if !path.is_dir() {
        return Ok(None);
    }

    if directory_contains_files(&path)? {
        bail!("directory contains files; forget files individually first: `{uri}`");
    }

    fs::remove_dir_all(&path)
        .with_context(|| format!("failed to delete directory `{}`", path.display()))?;

    Ok(Some(ForgetResult {
        uri: uri.to_string(),
        path: normalize_path(&path),
    }))
}

fn namespace_dir_path(namespace: Namespace, relative_path: &str) -> PathBuf {
    let base = match namespace {
        Namespace::User => USER_DIR,
        Namespace::Agent => AGENT_DIR,
        Namespace::Resources => unreachable!(),
    };

    PathBuf::from(base).join(relative_path)
}

fn directory_contains_files(path: &Path) -> Result<bool> {
    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read directory `{}`", path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect `{}`", entry_path.display()))?;

        if file_type.is_file() {
            return Ok(true);
        }

        if file_type.is_dir() && directory_contains_files(&entry_path)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
}
