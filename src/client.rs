use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};

use crate::config::WorkspaceConfig;
use crate::protocol::{ErrorResponse, ExecuteRequest, ExecuteResponse};
use crate::render;
use crate::timing::{log_timing, log_value};

pub fn execute(request: &ExecuteRequest) -> Result<String> {
    let total_start = Instant::now();
    let setup_start = Instant::now();
    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    let url = format!("http://127.0.0.1:{}/v1/execute", config.server_port);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    log_timing("client_setup", setup_start.elapsed());

    let http_start = Instant::now();
    let mut response = match agent.post(&url).send_json(request) {
        Ok(response) => response,
        Err(error) => {
            return Err(anyhow!(error.to_string())).with_context(|| {
                format!(
                    "failed to reach Memento server at 127.0.0.1:{}; run `memento serve` first",
                    config.server_port
                )
            });
        }
    };
    log_timing("client_http_round_trip", http_start.elapsed());
    log_value("client_http_status", response.status());

    if !response.status().is_success() {
        let error = response
            .body_mut()
            .read_json::<ErrorResponse>()
            .map(|payload| payload.error)
            .unwrap_or_else(|_| "server returned an error".to_string());
        bail!(error);
    }

    let decode_start = Instant::now();
    let payload: ExecuteResponse = response
        .body_mut()
        .read_json()
        .map_err(|error| anyhow!(error.to_string()))
        .context("failed to decode server response")?;
    log_timing("client_decode_response", decode_start.elapsed());

    let render_start = Instant::now();
    let output = match payload {
        ExecuteResponse::Add { result } => render::add::render(&result),
        ExecuteResponse::Doctor { result } => render::doctor::render(&result),
        ExecuteResponse::Forget { result } => render::forget::render(&result),
        ExecuteResponse::Cat { result } => render::cat::render(&result),
        ExecuteResponse::Find { result } => render::find::render(&result),
        ExecuteResponse::Ls { result } => render::ls::render(&result),
        ExecuteResponse::Models { result } => render::models::render(&result),
        ExecuteResponse::Reindex { result } => render::reindex::render(&result),
        ExecuteResponse::Remember { result } => render::remember::render(&result),
        ExecuteResponse::Rm { result } => render::rm::render(&result),
        ExecuteResponse::Show { result } => render::show::render(&result),
    };
    log_timing("client_render_output", render_start.elapsed());
    log_timing("client_total_before_print", total_start.elapsed());

    Ok(output)
}
