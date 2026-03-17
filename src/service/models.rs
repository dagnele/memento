use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::embedding::supported_embedding_profiles;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub dimension: usize,
    pub recommended: bool,
    pub use_case: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResult {
    pub models: Vec<ModelInfo>,
}

pub fn execute() -> Result<ModelsResult> {
    let models = supported_embedding_profiles()
        .iter()
        .map(|profile| ModelInfo {
            name: profile.name.to_string(),
            dimension: profile.dimension,
            recommended: profile.recommended,
            use_case: profile.use_case.to_string(),
        })
        .collect();

    Ok(ModelsResult { models })
}
