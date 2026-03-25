use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, ensure};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use crate::cli::EmbeddingModelArg;
use crate::config::WorkspaceConfig;
use crate::timing::log_timing;

#[derive(Debug, Clone)]
pub struct EmbeddingProfile {
    pub arg: EmbeddingModelArg,
    pub name: &'static str,
    pub dimension: usize,
    pub model: EmbeddingModel,
    pub recommended: bool,
    pub use_case: &'static str,
}

const EMBEDDING_PROFILES: [EmbeddingProfile; 6] = [
    EmbeddingProfile {
        arg: EmbeddingModelArg::BgeBaseEnV15,
        name: "bge-base-en-v1.5",
        dimension: 768,
        model: EmbeddingModel::BGEBaseENV15,
        recommended: true,
        use_case: "balanced default for English notes and docs",
    },
    EmbeddingProfile {
        arg: EmbeddingModelArg::BgeSmallEnV15,
        name: "bge-small-en-v1.5",
        dimension: 384,
        model: EmbeddingModel::BGESmallENV15,
        recommended: false,
        use_case: "fast lightweight indexing on local machines",
    },
    EmbeddingProfile {
        arg: EmbeddingModelArg::BgeLargeEnV15,
        name: "bge-large-en-v1.5",
        dimension: 1024,
        model: EmbeddingModel::BGELargeENV15,
        recommended: false,
        use_case: "highest-quality English retrieval",
    },
    EmbeddingProfile {
        arg: EmbeddingModelArg::JinaEmbeddingsV2BaseCode,
        name: "jina-embeddings-v2-base-code",
        dimension: 768,
        model: EmbeddingModel::JinaEmbeddingsV2BaseCode,
        recommended: false,
        use_case: "code-heavy repositories and source search",
    },
    EmbeddingProfile {
        arg: EmbeddingModelArg::NomicEmbedTextV15,
        name: "nomic-embed-text-v1.5",
        dimension: 768,
        model: EmbeddingModel::NomicEmbedTextV15,
        recommended: false,
        use_case: "longer English notes and general semantic search",
    },
    EmbeddingProfile {
        arg: EmbeddingModelArg::BgeM3,
        name: "bge-m3",
        dimension: 1024,
        model: EmbeddingModel::BGEM3,
        recommended: false,
        use_case: "multilingual content across mixed repositories",
    },
];

static EMBEDDER: OnceLock<Result<Mutex<TextEmbedding>, String>> = OnceLock::new();

pub fn embed_text(text: &str) -> Result<Vec<f32>> {
    if let Some(embedding) = test_embedding(text) {
        return Ok(embedding);
    }

    let embedder = get_embedder()?;
    let mut embedder = embedder
        .lock()
        .map_err(|_| anyhow!("embedding model lock poisoned"))?;
    let embeddings = embedder
        .embed(vec![text], None)
        .context("failed to generate embedding")?;
    let embedding = embeddings
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("embedding model returned no vectors"))?;
    let profile = current_embedding_profile()?;

    ensure!(
        embedding.len() == profile.dimension,
        "unexpected embedding dimension: expected {}, got {}",
        profile.dimension,
        embedding.len()
    );

    Ok(embedding)
}

fn test_embedding(text: &str) -> Option<Vec<f32>> {
    std::env::var_os("MEMENTO_TEST_EMBEDDING")?;

    let dimension = current_embedding_profile().ok()?.dimension;

    let mut embedding = vec![0.0_f32; dimension];

    for token in text.split_whitespace() {
        let mut hash = 2166136261_u32;

        for byte in token.as_bytes() {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(16777619);
        }

        let index = (hash as usize) % dimension;
        embedding[index] += 1.0;
    }

    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut embedding {
            *value /= norm;
        }
    }

    Some(embedding)
}

fn get_embedder() -> Result<&'static Mutex<TextEmbedding>> {
    let start = std::time::Instant::now();
    let embedder = EMBEDDER
        .get_or_init(|| {
            initialize_embedder()
                .map(Mutex::new)
                .map_err(|err| err.to_string())
        })
        .as_ref()
        .map_err(|err| anyhow!(err.clone()));
    log_timing("embedder_ready", start.elapsed());
    embedder
}

fn initialize_embedder() -> Result<TextEmbedding> {
    let cache_dir = embedding_cache_dir()?;
    let profile = current_embedding_profile()?;
    std::fs::create_dir_all(&cache_dir).with_context(|| {
        format!(
            "failed to create embedding cache directory `{}`",
            cache_dir.display()
        )
    })?;

    let options = InitOptions::new(profile.model)
        .with_cache_dir(cache_dir)
        .with_show_download_progress(true);

    TextEmbedding::try_new(options).context("failed to initialize local embedding model")
}

pub fn embedding_cache_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("MEMENTO_MODEL_CACHE_DIR") {
        return Ok(PathBuf::from(path));
    }

    let home =
        dirs::home_dir().ok_or_else(|| anyhow!("failed to determine user home directory"))?;
    Ok(home.join(".memento").join("models"))
}

pub fn default_embedding_profile() -> EmbeddingProfile {
    EMBEDDING_PROFILES[0].clone()
}

pub fn supported_embedding_profiles() -> &'static [EmbeddingProfile] {
    &EMBEDDING_PROFILES
}

pub fn embedding_profile_from_arg(arg: EmbeddingModelArg) -> EmbeddingProfile {
    EMBEDDING_PROFILES
        .iter()
        .find(|profile| profile.arg == arg)
        .cloned()
        .unwrap_or_else(default_embedding_profile)
}

pub fn embedding_profile_by_name(name: &str) -> Result<EmbeddingProfile> {
    EMBEDDING_PROFILES
        .iter()
        .find(|profile| profile.name == name)
        .cloned()
        .ok_or_else(|| anyhow!("unsupported embedding model `{name}`"))
}

pub fn current_embedding_profile() -> Result<EmbeddingProfile> {
    let config = WorkspaceConfig::load()?;
    embedding_profile_by_name(&config.embedding_model)
}
