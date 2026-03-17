use anyhow::{Result, bail};

use crate::embedding::current_embedding_profile;
use crate::repository::workspace::WorkspaceRepository;

pub fn validate_workspace_embedding(repository: &WorkspaceRepository) -> Result<()> {
    let profile = current_embedding_profile()?;
    let stored_model = repository.get_workspace_meta("embedding_model")?;
    let stored_dimension = repository.get_workspace_meta("embedding_dimension")?;

    if stored_model.as_deref() != Some(profile.name)
        || stored_dimension.as_deref() != Some(&profile.dimension.to_string())
    {
        bail!(
            "workspace embedding model does not match config; rerun `memento init --model {}` or fix `.memento/config.toml`",
            profile.name
        );
    }

    Ok(())
}
