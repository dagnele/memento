use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{INDEX_FILE, ItemRecord, WorkspaceRepository};
use crate::resource_state::detect_live_state;
use crate::uri::{self, Namespace, ParsedUri};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsEntry {
    pub uri: String,
    pub kind: String,
    pub live_state: Option<String>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LsResult {
    pub target: String,
    pub entries: Vec<LsEntry>,
}

pub fn execute(uri: Option<String>) -> Result<LsResult> {
    let target = uri.unwrap_or_else(|| crate::uri::ROOT_URI.to_string());
    let parsed = uri::parse_memento_uri(&target)?;

    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;
    let items = repository.list_items()?;

    let entries = build_entries(&parsed, &items)
        .into_iter()
        .map(|entry| LsEntry {
            uri: entry.uri,
            kind: match entry.kind {
                EntryKind::Directory => "dir".to_string(),
                EntryKind::File => "file".to_string(),
            },
            live_state: entry.live_state,
            source_path: entry.source_path,
        })
        .collect();

    Ok(LsResult { target, entries })
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct ListEntry {
    uri: String,
    kind: EntryKind,
    live_state: Option<String>,
    source_path: Option<String>,
}

fn build_entries(target: &ParsedUri, items: &[ItemRecord]) -> Vec<ListEntry> {
    match target {
        ParsedUri::Root => vec![
            namespace_entry(Namespace::Resources),
            namespace_entry(Namespace::User),
            namespace_entry(Namespace::Agent),
        ],
        ParsedUri::Namespace(Namespace::Resources) => build_resource_entries("", items),
        ParsedUri::Namespace(Namespace::User) => {
            build_namespace_entries(Namespace::User, "", items)
        }
        ParsedUri::Namespace(Namespace::Agent) => {
            build_namespace_entries(Namespace::Agent, "", items)
        }
        ParsedUri::Item {
            namespace: Namespace::Resources,
            relative_path,
        } => build_resource_entries(relative_path, items),
        ParsedUri::Item {
            namespace: Namespace::User,
            relative_path,
        } => build_namespace_entries(Namespace::User, relative_path, items),
        ParsedUri::Item {
            namespace: Namespace::Agent,
            relative_path,
        } => build_namespace_entries(Namespace::Agent, relative_path, items),
    }
}

fn namespace_entry(namespace: Namespace) -> ListEntry {
    ListEntry {
        uri: namespace.root_uri().to_string(),
        kind: EntryKind::Directory,
        live_state: None,
        source_path: None,
    }
}

fn build_resource_entries(prefix: &str, items: &[ItemRecord]) -> Vec<ListEntry> {
    let mut entries = BTreeMap::new();

    for item in items.iter().filter(|item| item.namespace == "resources") {
        if let Some(entry) = resource_to_entry(prefix, item) {
            entries.entry(entry.uri.clone()).or_insert(entry);
        }
    }

    entries.into_values().collect()
}

fn resource_to_entry(prefix: &str, item: &ItemRecord) -> Option<ListEntry> {
    let source_path = item.source_path.as_deref()?;
    let relative_path = if prefix.is_empty() {
        source_path
    } else {
        source_path.strip_prefix(prefix)?.strip_prefix('/')?
    };

    if relative_path.is_empty() {
        return Some(ListEntry {
            uri: item.uri.clone(),
            kind: EntryKind::File,
            live_state: detect_live_state(item)
                .ok()
                .map(|state| state.as_str().to_string()),
            source_path: item.source_path.clone(),
        });
    }

    if let Some((first_segment, _)) = relative_path.split_once('/') {
        let entry_uri = uri::build_resource_uri(&join_relative_path(prefix, first_segment));
        return Some(ListEntry {
            uri: entry_uri,
            kind: EntryKind::Directory,
            live_state: None,
            source_path: None,
        });
    }

    Some(ListEntry {
        uri: item.uri.clone(),
        kind: EntryKind::File,
        live_state: detect_live_state(item)
            .ok()
            .map(|state| state.as_str().to_string()),
        source_path: item.source_path.clone(),
    })
}

fn build_namespace_entries(
    namespace: Namespace,
    prefix: &str,
    items: &[ItemRecord],
) -> Vec<ListEntry> {
    let mut entries = BTreeMap::new();

    for item in items
        .iter()
        .filter(|item| item.namespace == namespace.as_str())
    {
        if let Some(entry) = namespace_item_to_entry(namespace, prefix, item) {
            entries.entry(entry.uri.clone()).or_insert(entry);
        }
    }

    entries.into_values().collect()
}

fn namespace_item_to_entry(
    namespace: Namespace,
    prefix: &str,
    item: &ItemRecord,
) -> Option<ListEntry> {
    let relative_path = item
        .uri
        .strip_prefix(namespace.root_uri())?
        .strip_prefix('/')?;

    let visible_path = if prefix.is_empty() {
        relative_path
    } else {
        relative_path.strip_prefix(prefix)?.strip_prefix('/')?
    };

    if visible_path.is_empty() {
        return Some(ListEntry {
            uri: item.uri.clone(),
            kind: EntryKind::File,
            live_state: detect_live_state(item)
                .ok()
                .map(|state| state.as_str().to_string()),
            source_path: item.source_path.clone(),
        });
    }

    if let Some((first_segment, _)) = visible_path.split_once('/') {
        let child_path = join_relative_path(prefix, first_segment);
        return Some(ListEntry {
            uri: format!("{}/{}", namespace.root_uri(), child_path),
            kind: EntryKind::Directory,
            live_state: None,
            source_path: None,
        });
    }

    Some(ListEntry {
        uri: item.uri.clone(),
        kind: EntryKind::File,
        live_state: detect_live_state(item)
            .ok()
            .map(|state| state.as_str().to_string()),
        source_path: item.source_path.clone(),
    })
}

fn join_relative_path(prefix: &str, child: &str) -> String {
    if prefix.is_empty() {
        child.to_string()
    } else {
        format!("{prefix}/{child}")
    }
}
