use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::repository::workspace::{
    ContentLayerRecord, INDEX_FILE, ItemRecord, WorkspaceRepository,
};
use crate::resource_state::detect_live_state;
use crate::uri::{self, Namespace, ParsedUri};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowLayer {
    pub layer: String,
    pub storage_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowItem {
    pub uri: String,
    pub namespace: String,
    pub kind: String,
    pub live_state: String,
    pub source_path: String,
    pub file_size_bytes: String,
    pub modified_at: String,
    pub layers: Vec<ShowLayer>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualEntry {
    pub uri: String,
    pub entry_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowResult {
    pub item: Option<ShowItem>,
    pub virtual_entry: Option<VirtualEntry>,
}

pub fn execute(uri: String) -> Result<ShowResult> {
    let parsed = uri::parse_memento_uri(&uri)?;
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;

    let item = repository.get_item_by_uri(&uri)?;

    if let Some(item) = item {
        let live_state = detect_live_state(&item)?;
        let layers = repository.list_content_layers(item.id)?;
        return Ok(ShowResult {
            item: Some(map_item(item, layers, live_state)),
            virtual_entry: None,
        });
    }

    Ok(ShowResult {
        item: None,
        virtual_entry: Some(VirtualEntry {
            uri: uri.clone(),
            entry_type: describe_virtual_entry(&parsed).to_string(),
        }),
    })
}

fn map_item(
    item: ItemRecord,
    layers: Vec<ContentLayerRecord>,
    live_state: crate::resource_state::LiveResourceState,
) -> ShowItem {
    ShowItem {
        uri: item.uri,
        namespace: item.namespace,
        kind: item.kind,
        live_state: live_state.as_str().to_string(),
        source_path: item.source_path.unwrap_or_else(|| "-".to_string()),
        file_size_bytes: item
            .file_size_bytes
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        modified_at: item.modified_at.unwrap_or_else(|| "-".to_string()),
        layers: layers
            .into_iter()
            .map(|layer| ShowLayer {
                layer: layer.layer,
                storage_kind: layer.storage_kind,
            })
            .collect(),
        created_at: item.created_at,
        updated_at: item.updated_at,
    }
}

fn describe_virtual_entry(parsed: &ParsedUri) -> &'static str {
    match parsed {
        ParsedUri::Root => "root",
        ParsedUri::Namespace(Namespace::Resources) => "namespace",
        ParsedUri::Namespace(Namespace::User) => "namespace",
        ParsedUri::Namespace(Namespace::Agent) => "namespace",
        ParsedUri::Item {
            namespace: Namespace::Resources,
            ..
        } => "resource_dir",
        ParsedUri::Item { .. } => "namespace_dir",
    }
}
