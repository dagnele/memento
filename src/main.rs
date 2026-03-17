mod bootstrap;
mod cli;
mod client;
mod config;
mod dispatch;
mod embedding;
mod indexing;
mod mcp;
mod protocol;
mod render;
mod repository;
mod resource_state;
mod server;
mod service;
mod text_file;
mod timing;
mod uri;

use anyhow::Result;
use clap::Parser;
use std::time::Instant;

use crate::cli::{Cli, CliCommand};
use crate::protocol::ExecuteRequest;
use crate::timing::log_timing;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        CliCommand::Init { model, port } => {
            let output = bootstrap::init(model, port)?;
            println!("{output}");
            Ok(())
        }
        CliCommand::Models => {
            let result = service::models::execute()?;
            let output = render::models::render(&result);
            println!("{output}");
            Ok(())
        }
        CliCommand::Serve { debug } => server::serve(debug),
        command => {
            let total_start = Instant::now();
            let request = ExecuteRequest::try_from(command).map_err(anyhow::Error::msg)?;
            let output = client::execute(&request)?;
            let print_start = Instant::now();
            println!("{output}");
            log_timing("client_stdout_print", print_start.elapsed());
            log_timing("client_end_to_end", total_start.elapsed());
            Ok(())
        }
    }
}
