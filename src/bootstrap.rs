use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::cli::EmbeddingModelArg;
use crate::config::WorkspaceConfig;
use crate::embedding::{EmbeddingProfile, default_embedding_profile, embedding_profile_from_arg};
use crate::repository::workspace::{
    AGENT_DIR, INDEX_FILE, USER_DIR, WORKSPACE_DIR, WorkspaceRepository,
};

const DEFAULT_AGENT_SKILL_PATH: &str = ".memento/agent/skills/memento.md";
const DEFAULT_AGENT_SKILL_CONTENT: &str = r#"# Memento

Use `memento` to store and search local project knowledge.

## Using the CLI

- Run `memento serve` before using server-backed commands
- Use `memento add <path>...` to index project files under `mem://resources/...`
- Use `memento ls`, `memento show <uri>`, and `memento cat <uri>` to browse stored content
- Use `memento find <query>` to search indexed content semantically
- Use `memento remember --namespace user|agent --path <path> <text>` to save durable notes
- Use `memento forget <uri>` to remove a saved memory entry

## Using the MCP server

- Start `memento serve` to expose both the local API and the MCP endpoint
- The MCP endpoint shares the same `http://127.0.0.1:<server_port>` address
- Use MCP tools for actions like `add`, `remember`, `forget`, `show`, and `find`
- Use MCP resources for `mem://...` content, and resource templates like `mem://resources/{path}`

### MCP tools

- `doctor`
- `models`
- `add`
- `rm`
- `remember`
- `reindex`
- `forget`
- `ls`
- `cat`
- `show`
- `find`

## Core commands

- `memento add <path>...` indexes local files and directories under `mem://resources/...`
- `memento ls [uri]` lists resources and memory entries
- `memento show <uri>` shows metadata for a resource or memory entry
- `memento cat <uri>` reads the current contents of a resource or memory entry
- `memento find <query>` searches indexed content semantically

## Memory commands

- `memento remember --namespace user --path <path> <text>` stores user memory under `mem://user/...`
- `memento remember --namespace agent --path <path> <text>` stores agent memory under `mem://agent/...`
- `memento forget <uri>` removes a stored memory entry

## Notes

- Start the local server with `memento serve`
- The MCP endpoint is available on the same server port
- Do not add files from `.memento/` with `memento add`
"#;

pub fn init(model: Option<EmbeddingModelArg>, port: Option<u16>) -> Result<String> {
    let profile = model
        .map(embedding_profile_from_arg)
        .unwrap_or_else(default_embedding_profile);
    let server_port = port.unwrap_or_else(|| WorkspaceConfig::default().server_port);

    create_dir(WORKSPACE_DIR)?;
    create_dir(USER_DIR)?;
    create_dir(AGENT_DIR)?;
    write_config(&profile, server_port)?;
    initialize_database(INDEX_FILE, &profile)?;
    write_default_agent_skill()?;

    let lines = [
        format!(
            "{} {} {}",
            "memento".bold(),
            "init".cyan().bold(),
            "workspace initialized".green()
        ),
        format!("{} {}", "workspace".dimmed(), WORKSPACE_DIR.cyan()),
        format!(
            "{} {} {}",
            "embedding_model".dimmed(),
            profile.name.cyan(),
            format!("dim={}", profile.dimension).dimmed()
        ),
        format!("{} {}", "server_port".dimmed(), server_port),
    ];

    Ok(lines.join("\n"))
}

fn create_dir(path: &str) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create directory `{path}`"))
}

fn write_config(profile: &EmbeddingProfile, server_port: u16) -> Result<()> {
    let config = WorkspaceConfig::with_embedding_profile_and_port(profile.clone(), server_port);
    config.write()
}

fn initialize_database(path: &str, profile: &EmbeddingProfile) -> Result<()> {
    let repository = WorkspaceRepository::open(path)?;
    repository
        .initialize_schema()
        .with_context(|| format!("failed to initialize database schema in `{path}`"))?;
    repository.initialize_vector_schema(profile.dimension)?;
    repository.set_workspace_meta("embedding_model", profile.name)?;
    repository.set_workspace_meta("embedding_dimension", &profile.dimension.to_string())?;
    Ok(())
}

pub fn ensure_default_agent_skill_file() -> Result<()> {
    write_default_agent_skill()
}

fn write_default_agent_skill() -> Result<()> {
    let skill_path = Path::new(DEFAULT_AGENT_SKILL_PATH);

    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory `{}`", parent.display()))?;
    }

    fs::write(skill_path, DEFAULT_AGENT_SKILL_CONTENT)
        .with_context(|| format!("failed to write `{}`", skill_path.display()))?;

    Ok(())
}
