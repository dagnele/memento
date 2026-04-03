use anyhow::Result;

use crate::protocol::{ExecuteResponse, RemoteCommand};
use crate::service;

pub fn execute_remote_structured(command: RemoteCommand) -> Result<ExecuteResponse> {
    match command {
        RemoteCommand::Add { force, paths } => Ok(ExecuteResponse::Add {
            result: service::add::execute(force, paths)?,
        }),
        RemoteCommand::Doctor => Ok(ExecuteResponse::Doctor {
            result: service::doctor::execute()?,
        }),
        RemoteCommand::Forget { uri } => Ok(ExecuteResponse::Forget {
            result: service::forget::execute(uri)?,
        }),
        RemoteCommand::Cat { uri } => Ok(ExecuteResponse::Cat {
            result: service::cat::execute(uri)?,
        }),
        RemoteCommand::Find { query } => Ok(ExecuteResponse::Find {
            result: service::find::execute(query)?,
        }),
        RemoteCommand::Ls { uri } => Ok(ExecuteResponse::Ls {
            result: service::ls::execute(uri)?,
        }),
        RemoteCommand::Models => Ok(ExecuteResponse::Models {
            result: service::models::execute()?,
        }),
        RemoteCommand::Reindex { paths } => Ok(ExecuteResponse::Reindex {
            result: service::reindex::execute(paths)?,
        }),
        RemoteCommand::Rm { target } => Ok(ExecuteResponse::Rm {
            result: service::rm::execute(target)?,
        }),
        RemoteCommand::Remember { uri, file, text } => Ok(ExecuteResponse::Remember {
            result: service::remember::execute(uri, file, text)?,
        }),
        RemoteCommand::Show { uri } => Ok(ExecuteResponse::Show {
            result: Box::new(service::show::execute(uri)?),
        }),
    }
}
