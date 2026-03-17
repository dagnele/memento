use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::{CONFIG_FILE, WorkspaceConfig};
use crate::embedding::{current_embedding_profile, embedding_cache_dir};
use crate::repository::workspace::{
    AGENT_DIR, INDEX_FILE, USER_DIR, WORKSPACE_DIR, WorkspaceRepository,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub label: String,
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorConfigStatus {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorResult {
    pub checks: Vec<DoctorCheck>,
    pub config_status: DoctorConfigStatus,
    pub item_count_status: String,
    pub workspace_embedding_status: String,
    pub active_embedding_status: String,
    pub test_embedding_status: String,
}

pub fn execute() -> Result<DoctorResult> {
    let workspace = PathBuf::from(WORKSPACE_DIR);
    let config = PathBuf::from(CONFIG_FILE);
    let index = PathBuf::from(INDEX_FILE);
    let user_dir = PathBuf::from(USER_DIR);
    let agent_dir = PathBuf::from(AGENT_DIR);
    let cache_dir = embedding_cache_dir()?;

    let checks = vec![
        build_check("workspace", &workspace),
        build_check("config", &config),
        build_check("index", &index),
        build_check("user_dir", &user_dir),
        build_check("agent_dir", &agent_dir),
        build_check("model_cache", &cache_dir),
    ];

    let config_status = match WorkspaceConfig::load() {
        Ok(config) => DoctorConfigStatus {
            ok: true,
            message: format!(
                "version={} model={} dim={} segment_lines={} overlap={}",
                config.workspace_version,
                config.embedding_model,
                config.embedding_dimension,
                config.segment_line_count,
                config.segment_line_overlap
            ),
        },
        Err(error) => DoctorConfigStatus {
            ok: false,
            message: error.to_string(),
        },
    };

    let (item_count_status, workspace_embedding_status) =
        match WorkspaceRepository::open(INDEX_FILE).and_then(|repository| {
            repository.initialize_schema()?;
            let count = repository.item_count()?;
            let stored_model = repository.get_workspace_meta("embedding_model")?;
            let stored_dimension = repository.get_workspace_meta("embedding_dimension")?;
            Ok((count, stored_model, stored_dimension))
        }) {
            Ok((count, model, dimension)) => (
                count.to_string(),
                format!(
                    "model={} dim={}",
                    model.as_deref().unwrap_or("-"),
                    dimension.as_deref().unwrap_or("-")
                ),
            ),
            Err(error) => (format!("error {error}"), "error -".to_string()),
        };

    let active_embedding_status = match current_embedding_profile() {
        Ok(profile) => format!(
            "{} dim={}{}",
            profile.name,
            profile.dimension,
            if profile.recommended {
                " recommended"
            } else {
                ""
            }
        ),
        Err(error) => format!("error {error}"),
    };

    let test_embedding_status = if std::env::var_os("MEMENTO_TEST_EMBEDDING").is_some() {
        "enabled".to_string()
    } else {
        "disabled".to_string()
    };

    Ok(DoctorResult {
        checks,
        config_status,
        item_count_status,
        workspace_embedding_status,
        active_embedding_status,
        test_embedding_status,
    })
}

fn build_check(label: &str, path: &std::path::Path) -> DoctorCheck {
    DoctorCheck {
        label: label.to_string(),
        status: if path.exists() {
            "ok".to_string()
        } else {
            "missing".to_string()
        },
        path: path.display().to_string(),
    }
}
