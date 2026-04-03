use serde::{Deserialize, Serialize};

use crate::cli::{CliCommand, EmbeddingModelArg};
use crate::service::add::AddResult;
use crate::service::cat::CatResult;
use crate::service::doctor::DoctorResult;
use crate::service::find::FindResult;
use crate::service::forget::ForgetResult;
use crate::service::ls::LsResult;
use crate::service::models::ModelsResult;
use crate::service::reindex::ReindexResult;
use crate::service::remember::RememberResult;
use crate::service::rm::RmResult;
use crate::service::show::ShowResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub command: RemoteCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum RemoteCommand {
    Doctor,
    Models,
    Add {
        force: bool,
        paths: Vec<String>,
    },
    Rm {
        target: String,
    },
    Remember {
        uri: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecuteResponse {
    Add { result: AddResult },
    Doctor { result: DoctorResult },
    Forget { result: ForgetResult },
    Find { result: FindResult },
    Ls { result: LsResult },
    Cat { result: CatResult },
    Models { result: ModelsResult },
    Reindex { result: ReindexResult },
    Remember { result: RememberResult },
    Rm { result: RmResult },
    Show { result: Box<ShowResult> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl TryFrom<CliCommand> for ExecuteRequest {
    type Error = &'static str;

    fn try_from(command: CliCommand) -> Result<Self, Self::Error> {
        let command = match command {
            CliCommand::Doctor => RemoteCommand::Doctor,
            CliCommand::Models => RemoteCommand::Models,
            CliCommand::Add { force, paths } => RemoteCommand::Add { force, paths },
            CliCommand::Rm { target } => RemoteCommand::Rm { target },
            CliCommand::Remember { uri, file, text } => RemoteCommand::Remember { uri, file, text },
            CliCommand::Reindex { paths } => RemoteCommand::Reindex { paths },
            CliCommand::Forget { uri } => RemoteCommand::Forget { uri },
            CliCommand::Ls { uri } => RemoteCommand::Ls { uri },
            CliCommand::Cat { uri } => RemoteCommand::Cat { uri },
            CliCommand::Show { uri } => RemoteCommand::Show { uri },
            CliCommand::Find { query } => RemoteCommand::Find { query },
            CliCommand::Init { .. } | CliCommand::Serve { .. } | CliCommand::Mcp { .. } => {
                return Err("command is not executed through the server");
            }
        };

        Ok(Self { command })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitConfigPayload {
    pub model: Option<EmbeddingModelArg>,
    pub port: Option<u16>,
}
