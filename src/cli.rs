use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "memento", version)]
#[command(about = "A fast, local CLI for context indexing")]
#[command(
    long_about = "Index local project files, store durable user or agent notes, and search them through a small local workspace.\n\nMost commands talk to the local Memento server. Initialize once with `memento init`, then run `memento serve` in the workspace before using indexing and search commands."
)]
#[command(
    after_help = "Quick start:\n  memento init\n  memento serve\n  memento add \"docs/**/*.md\"\n  memento find \"release checklist\"\n  memento remember mem://user/preferences/style.md \"Prefer concise answers\""
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, Subcommand)]
pub enum CliCommand {
    #[command(about = "Create a .memento workspace in the current directory")]
    Init {
        #[arg(
            long,
            value_enum,
            help = "Embedding model to use; see `memento models` for options and use cases"
        )]
        model: Option<EmbeddingModelArg>,
        #[arg(long, help = "Server port to store in the generated workspace config")]
        port: Option<u16>,
    },
    #[command(about = "Run the local server used by indexing and search commands")]
    Serve {
        #[arg(long, help = "Print per-command debug timing to stderr")]
        debug: bool,
        #[arg(
            long,
            help = "Path to the project directory containing the .memento workspace"
        )]
        dir: Option<PathBuf>,
    },
    #[command(about = "Run the MCP server over stdio or HTTP")]
    Mcp {
        #[arg(
            long,
            value_enum,
            default_value = "stdio",
            help = "Transport to use for the MCP server"
        )]
        transport: McpTransport,
        #[arg(long, help = "Port to bind when using HTTP transport")]
        port: Option<u16>,
        #[arg(
            long,
            help = "Path to the project directory containing the .memento workspace"
        )]
        dir: Option<PathBuf>,
    },
    #[command(about = "Check workspace, config, index, and embedding setup")]
    Doctor,
    #[command(about = "List supported embedding models and their use cases")]
    Models,
    #[command(about = "Index local text files into mem://resources")]
    Add {
        #[arg(long, help = "Re-index paths even if they were already added before")]
        force: bool,
        #[arg(help = "File paths or glob patterns to index, for example `notes/*.md`")]
        paths: Vec<String>,
    },
    #[command(about = "Untrack an indexed resource without deleting the source file")]
    Rm {
        #[arg(help = "Tracked resource path or mem://resources URI to remove from the index")]
        target: String,
    },
    #[command(
        about = "Store a user or agent memory item",
        long_about = "Store a user or agent memory item under a Memento URI. Use `mem://user/...` or `mem://agent/...`. Inline text requires a destination URI ending in `.md`. File imports can use any text file extension."
    )]
    Remember {
        #[arg(
            help = "Destination memory URI, for example `mem://user/preferences/style.md` for inline text"
        )]
        uri: String,
        #[arg(long, help = "Read item contents from a UTF-8 text file")]
        file: Option<String>,
        #[arg(help = "Inline text to store; requires the destination URI to end in `.md`")]
        text: Option<String>,
    },
    #[command(about = "Refresh indexed content for previously added resources")]
    Reindex {
        #[arg(help = "Resource paths to refresh; omit to refresh all tracked resources")]
        paths: Vec<String>,
    },
    #[command(about = "Remove a stored memory item or an empty memory directory")]
    Forget {
        #[arg(help = "Memory URI to remove, for example `mem://agent/notes/todo.md`")]
        uri: String,
    },
    #[command(about = "List resources and memory items by URI prefix")]
    Ls {
        #[arg(help = "Optional URI prefix such as `mem://resources` or `mem://user/preferences`")]
        uri: Option<String>,
    },
    #[command(about = "Print the contents of a resource or memory item")]
    Cat {
        #[arg(help = "Resource or memory URI to read")]
        uri: String,
    },
    #[command(about = "Show metadata for a resource or memory item")]
    Show {
        #[arg(help = "Resource or memory URI to inspect")]
        uri: String,
    },
    #[command(about = "Search indexed resources and memory by semantic similarity")]
    Find {
        #[arg(help = "Natural-language query to search for")]
        query: String,
    },
}

impl CliCommand {
    pub fn label(&self) -> &'static str {
        match self {
            CliCommand::Init { .. } => "init",
            CliCommand::Serve { .. } => "serve",
            CliCommand::Mcp { .. } => "mcp",
            CliCommand::Doctor => "doctor",
            CliCommand::Models => "models",
            CliCommand::Add { .. } => "add",
            CliCommand::Rm { .. } => "rm",
            CliCommand::Remember { .. } => "remember",
            CliCommand::Reindex { .. } => "reindex",
            CliCommand::Forget { .. } => "forget",
            CliCommand::Ls { .. } => "ls",
            CliCommand::Cat { .. } => "cat",
            CliCommand::Show { .. } => "show",
            CliCommand::Find { .. } => "find",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
pub enum McpTransport {
    Stdio,
    Http,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
pub enum EmbeddingModelArg {
    #[value(name = "bge-small-en-v1.5")]
    BgeSmallEnV15,
    #[value(name = "bge-base-en-v1.5")]
    BgeBaseEnV15,
    #[value(name = "bge-large-en-v1.5")]
    BgeLargeEnV15,
    #[value(name = "jina-embeddings-v2-base-code")]
    JinaEmbeddingsV2BaseCode,
    #[value(name = "nomic-embed-text-v1.5")]
    NomicEmbedTextV15,
    #[value(name = "bge-m3")]
    BgeM3,
}
