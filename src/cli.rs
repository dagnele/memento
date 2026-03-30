use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "memento", version)]
#[command(about = "A fast, local CLI for context indexing")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, Subcommand)]
pub enum CliCommand {
    Init {
        #[arg(
            long,
            value_enum,
            help = "Embedding model to use; see `memento models` for options and use cases"
        )]
        model: Option<EmbeddingModelArg>,
        #[arg(long)]
        port: Option<u16>,
    },
    Serve {
        #[arg(long, help = "Print per-command debug timing to stderr")]
        debug: bool,
        #[arg(
            long,
            help = "Path to the project directory containing the .memento workspace"
        )]
        dir: Option<PathBuf>,
    },
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
    Doctor,
    Models,
    Add {
        #[arg(long)]
        force: bool,
        paths: Vec<String>,
    },
    Rm {
        target: String,
    },
    Remember {
        #[arg(long, value_enum)]
        namespace: MemoryNamespace,
        #[arg(long)]
        path: String,
        #[arg(long)]
        file: Option<String>,
        text: Option<String>,
    },
    Reindex {
        paths: Vec<String>,
    },
    Forget {
        uri: String,
    },
    Ls {
        uri: Option<String>,
    },
    Cat {
        uri: String,
    },
    Show {
        uri: String,
    },
    Find {
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum, Serialize, Deserialize, JsonSchema)]
pub enum MemoryNamespace {
    User,
    Agent,
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
