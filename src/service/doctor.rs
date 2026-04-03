use std::path::PathBuf;

use anyhow::{Context, Result};
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
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorResult {
    pub checks: Vec<DoctorCheck>,
    pub healthy: bool,
}

pub fn execute() -> Result<DoctorResult> {
    let workspace = PathBuf::from(WORKSPACE_DIR);
    let config = PathBuf::from(CONFIG_FILE);
    let index = PathBuf::from(INDEX_FILE);
    let user_dir = PathBuf::from(USER_DIR);
    let agent_dir = PathBuf::from(AGENT_DIR);
    let cache_dir = embedding_cache_dir()?;

    let mut checks = vec![
        build_path_check("workspace", &workspace, true),
        build_path_check("config", &config, true),
        build_path_check("index", &index, true),
        build_path_check("user_dir", &user_dir, true),
        build_path_check("agent_dir", &agent_dir, true),
        build_path_check("model_cache", &cache_dir, false),
    ];

    let config = match WorkspaceConfig::load() {
        Ok(config) => {
            checks.push(DoctorCheck {
                label: "config_load".to_string(),
                status: "ok".to_string(),
                detail: format!(
                    "version={} model={} dim={} segment_lines={} overlap={}",
                    config.workspace_version,
                    config.embedding_model,
                    config.embedding_dimension,
                    config.segment_line_count,
                    config.segment_line_overlap
                ),
            });
            Some(config)
        }
        Err(error) => {
            checks.push(DoctorCheck {
                label: "config_load".to_string(),
                status: "fail".to_string(),
                detail: error.to_string(),
            });
            None
        }
    };

    let active_profile = match current_embedding_profile() {
        Ok(profile) => {
            checks.push(DoctorCheck {
                label: "embedding_active".to_string(),
                status: if profile.recommended {
                    "ok".to_string()
                } else {
                    "warn".to_string()
                },
                detail: format!(
                    "model={} dim={}{}",
                    profile.name,
                    profile.dimension,
                    if profile.recommended {
                        " recommended"
                    } else {
                        ""
                    }
                ),
            });
            Some(profile)
        }
        Err(error) => {
            checks.push(DoctorCheck {
                label: "embedding_active".to_string(),
                status: "fail".to_string(),
                detail: error.to_string(),
            });
            None
        }
    };

    if !index.exists() {
        let message = format!("required path is missing: {}", index.display());
        checks.push(DoctorCheck {
            label: "index_open".to_string(),
            status: "fail".to_string(),
            detail: message.clone(),
        });
        checks.push(DoctorCheck {
            label: "index_schema".to_string(),
            status: "fail".to_string(),
            detail: message.clone(),
        });
        checks.push(DoctorCheck {
            label: "embedding_workspace".to_string(),
            status: "fail".to_string(),
            detail: message.clone(),
        });
        checks.push(DoctorCheck {
            label: "item_count".to_string(),
            status: "fail".to_string(),
            detail: message.clone(),
        });
        checks.push(DoctorCheck {
            label: "embedding_match".to_string(),
            status: "fail".to_string(),
            detail: message,
        });
    } else {
        match inspect_index() {
            Ok(index_status) => {
                checks.push(DoctorCheck {
                    label: "index_open".to_string(),
                    status: "ok".to_string(),
                    detail: INDEX_FILE.to_string(),
                });
                checks.push(DoctorCheck {
                    label: "index_schema".to_string(),
                    status: index_status.schema_status,
                    detail: index_status.schema_detail,
                });
                checks.push(DoctorCheck {
                    label: "embedding_workspace".to_string(),
                    status: index_status.embedding_status,
                    detail: index_status.embedding_detail,
                });
                checks.push(DoctorCheck {
                    label: "item_count".to_string(),
                    status: "ok".to_string(),
                    detail: index_status.item_count.to_string(),
                });

                checks.push(
                    match embedding_match_check(
                        config.as_ref(),
                        active_profile.as_ref(),
                        index_status.stored_model.as_deref(),
                        index_status.stored_dimension,
                    ) {
                        Ok(check) => check,
                        Err(error) => DoctorCheck {
                            label: "embedding_match".to_string(),
                            status: "fail".to_string(),
                            detail: error.to_string(),
                        },
                    },
                );
            }
            Err(error) => {
                let message = error.to_string();
                checks.push(DoctorCheck {
                    label: "index_open".to_string(),
                    status: "fail".to_string(),
                    detail: message.clone(),
                });
                checks.push(DoctorCheck {
                    label: "index_schema".to_string(),
                    status: "fail".to_string(),
                    detail: message.clone(),
                });
                checks.push(DoctorCheck {
                    label: "embedding_workspace".to_string(),
                    status: "fail".to_string(),
                    detail: message.clone(),
                });
                checks.push(DoctorCheck {
                    label: "item_count".to_string(),
                    status: "fail".to_string(),
                    detail: message.clone(),
                });
                checks.push(DoctorCheck {
                    label: "embedding_match".to_string(),
                    status: "fail".to_string(),
                    detail: message,
                });
            }
        }
    }

    checks.push(DoctorCheck {
        label: "test_embedding".to_string(),
        status: "info".to_string(),
        detail: if std::env::var_os("MEMENTO_TEST_EMBEDDING").is_some() {
            "enabled".to_string()
        } else {
            "disabled".to_string()
        },
    });

    let healthy = checks.iter().all(|check| check.status != "fail");

    Ok(DoctorResult { checks, healthy })
}

fn build_path_check(label: &str, path: &std::path::Path, required: bool) -> DoctorCheck {
    DoctorCheck {
        label: label.to_string(),
        status: if path.exists() {
            "ok".to_string()
        } else if required {
            "fail".to_string()
        } else {
            "warn".to_string()
        },
        detail: path.display().to_string(),
    }
}

struct IndexStatus {
    item_count: i64,
    schema_status: String,
    schema_detail: String,
    embedding_status: String,
    embedding_detail: String,
    stored_model: Option<String>,
    stored_dimension: Option<usize>,
}

fn inspect_index() -> Result<IndexStatus> {
    let repository = WorkspaceRepository::open(INDEX_FILE)
        .with_context(|| format!("failed to open database `{INDEX_FILE}`"))?;
    let schema_version = repository
        .get_workspace_meta("schema_version")
        .context("failed to read workspace metadata `schema_version`")?;
    let stored_model = repository
        .get_workspace_meta("embedding_model")
        .context("failed to read workspace metadata `embedding_model`")?;
    let stored_dimension_raw = repository
        .get_workspace_meta("embedding_dimension")
        .context("failed to read workspace metadata `embedding_dimension`")?;
    let item_count = repository.item_count()?;

    let (schema_status, schema_detail) = match schema_version {
        Some(version) => ("ok".to_string(), format!("version={version}")),
        None => (
            "fail".to_string(),
            "workspace metadata `schema_version` is missing".to_string(),
        ),
    };

    let stored_dimension = match stored_dimension_raw {
        Some(value) => Some(value.parse::<usize>().with_context(|| {
            format!("failed to parse workspace metadata `embedding_dimension`: `{value}`")
        })?),
        None => None,
    };

    let (embedding_status, embedding_detail) = match (&stored_model, stored_dimension) {
        (Some(model), Some(dimension)) => {
            ("ok".to_string(), format!("model={model} dim={dimension}"))
        }
        (model, dimension) => (
            "fail".to_string(),
            format!(
                "model={} dim={}",
                model.as_deref().unwrap_or("-"),
                dimension
                    .map(|value| value.to_string())
                    .as_deref()
                    .unwrap_or("-")
            ),
        ),
    };

    Ok(IndexStatus {
        item_count,
        schema_status,
        schema_detail,
        embedding_status,
        embedding_detail,
        stored_model,
        stored_dimension,
    })
}

fn embedding_match_check(
    config: Option<&WorkspaceConfig>,
    active_profile: Option<&crate::embedding::EmbeddingProfile>,
    stored_model: Option<&str>,
    stored_dimension: Option<usize>,
) -> Result<DoctorCheck> {
    let Some(config) = config else {
        return Ok(DoctorCheck {
            label: "embedding_match".to_string(),
            status: "fail".to_string(),
            detail: "config is unavailable".to_string(),
        });
    };

    let Some(active_profile) = active_profile else {
        return Ok(DoctorCheck {
            label: "embedding_match".to_string(),
            status: "fail".to_string(),
            detail: "active embedding profile is unavailable".to_string(),
        });
    };

    if stored_model == Some(config.embedding_model.as_str())
        && stored_dimension == Some(config.embedding_dimension)
        && stored_model == Some(active_profile.name)
        && stored_dimension == Some(active_profile.dimension)
    {
        return Ok(DoctorCheck {
            label: "embedding_match".to_string(),
            status: "ok".to_string(),
            detail: "workspace matches active profile".to_string(),
        });
    }

    Ok(DoctorCheck {
        label: "embedding_match".to_string(),
        status: "fail".to_string(),
        detail: format!(
            "config model={} dim={}, workspace model={} dim={}, active model={} dim={}",
            config.embedding_model,
            config.embedding_dimension,
            stored_model.unwrap_or("-"),
            stored_dimension
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            active_profile.name,
            active_profile.dimension
        ),
    })
}
