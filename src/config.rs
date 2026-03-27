use std::fs;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::embedding::{EmbeddingProfile, default_embedding_profile, embedding_profile_by_name};

pub const CONFIG_FILE: &str = ".memento/config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub workspace_version: u32,
    pub embedding_model: String,
    pub embedding_dimension: usize,
    pub segment_line_count: usize,
    pub segment_line_overlap: usize,
    pub server_port: u16,
}

impl WorkspaceConfig {
    pub fn load() -> Result<Self> {
        let contents = fs::read_to_string(CONFIG_FILE)
            .with_context(|| format!("failed to read config `{CONFIG_FILE}`"))?;
        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config `{CONFIG_FILE}`"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn write(&self) -> Result<()> {
        self.validate()?;
        let contents = toml::to_string(self).context("failed to serialize workspace config")?;
        fs::write(CONFIG_FILE, contents)
            .with_context(|| format!("failed to create config `{CONFIG_FILE}`"))
    }

    pub fn with_embedding_profile(profile: EmbeddingProfile) -> Self {
        Self {
            embedding_model: profile.name.to_string(),
            embedding_dimension: profile.dimension,
            ..Self::default()
        }
    }

    pub fn with_embedding_profile_and_port(profile: EmbeddingProfile, server_port: u16) -> Self {
        Self {
            server_port,
            ..Self::with_embedding_profile(profile)
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.segment_line_count > 0,
            "segment_line_count must be greater than 0"
        );
        ensure!(
            self.segment_line_overlap < self.segment_line_count,
            "segment_line_overlap must be less than segment_line_count"
        );
        let profile = embedding_profile_by_name(&self.embedding_model)?;
        ensure!(
            self.embedding_dimension == profile.dimension,
            "embedding_dimension must match model `{}` (expected {})",
            profile.name,
            profile.dimension
        );
        ensure!(self.server_port > 0, "server_port must be greater than 0");
        Ok(())
    }
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        let profile = default_embedding_profile();
        Self {
            workspace_version: 4,
            embedding_model: profile.name.to_string(),
            embedding_dimension: profile.dimension,
            segment_line_count: 40,
            segment_line_overlap: 10,
            server_port: 4000,
        }
    }
}
