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
mod spinner;
mod text_file;
mod timing;
mod uri;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crate::cli::{Cli, CliCommand};
use crate::protocol::ExecuteRequest;
use crate::timing::log_timing;

fn change_dir(dir: Option<PathBuf>) -> Result<()> {
    if let Some(dir) = dir {
        std::env::set_current_dir(&dir)
            .with_context(|| format!("failed to change directory to `{}`", dir.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    match cli.command {
        CliCommand::Init { model, port } => {
            let output = bootstrap::init(model, port)?;
            println!("{output}");
            Ok(ExitCode::SUCCESS)
        }
        CliCommand::Models => {
            let result = service::models::execute()?;
            let output = render::models::render(&result);
            println!("{output}");
            Ok(ExitCode::SUCCESS)
        }
        CliCommand::Serve { debug, dir } => {
            change_dir(dir)?;
            server::serve(debug)?;
            Ok(ExitCode::SUCCESS)
        }
        CliCommand::Mcp {
            transport,
            port,
            dir,
        } => {
            change_dir(dir)?;
            match transport {
                cli::McpTransport::Stdio => mcp::serve_stdio()?,
                cli::McpTransport::Http => mcp::serve_http(port)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        command => {
            let total_start = Instant::now();
            let label = command.label();
            let request = ExecuteRequest::try_from(command).map_err(anyhow::Error::msg)?;
            let spinner = spinner::Spinner::start(label);
            let result = client::execute(&request);
            spinner.stop();
            let output = result?;
            let print_start = Instant::now();
            println!("{}", output.stdout);
            log_timing("client_stdout_print", print_start.elapsed());
            log_timing("client_end_to_end", total_start.elapsed());
            Ok(if output.exit_ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
