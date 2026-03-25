use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::bootstrap;
use crate::config::WorkspaceConfig;
use crate::dispatch;
use crate::indexing::index_namespace_item;
use crate::mcp;
use crate::protocol::{ErrorResponse, ExecuteRequest, RemoteCommand};
use crate::repository::workspace::{AGENT_DIR, INDEX_FILE, USER_DIR, WorkspaceRepository};
use crate::timing::{enable_timing, timing_enabled};
use crate::uri::Namespace;

pub fn serve(debug: bool) -> Result<()> {
    if debug {
        enable_timing();
    }

    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    let address = format!("127.0.0.1:{}", config.server_port);
    let mcp_port = config.server_port + 1;
    let mcp_address = format!("127.0.0.1:{mcp_port}");
    let server = Server::http(&address)
        .map_err(|error| anyhow!(error.to_string()))
        .with_context(|| format!("failed to bind server on {address}"))?;
    let mcp_server = mcp::spawn_http_server(mcp_port)
        .with_context(|| format!("failed to start MCP server on {mcp_address}"))?;
    ensure_namespace_items_indexed(&config)?;
    let write_lock = Mutex::new(());

    println!("memento serve listening on http://{address}");
    println!("memento mcp listening on http://{mcp_address}");

    if debug {
        eprintln!("[memento:debug] server debug logging enabled");
    }

    for mut request in server.incoming_requests() {
        let response = match (request.method(), request.url()) {
            (&Method::Get, "/health") => {
                json_response(StatusCode(200), &serde_json::json!({ "status": "ok" }))
            }
            (&Method::Post, "/v1/execute") => handle_execute(&mut request, &write_lock),
            _ => json_error(StatusCode(404), "not found"),
        };

        let _ = request.respond(response);
    }

    mcp_server.shutdown();

    Ok(())
}

fn ensure_namespace_items_indexed(config: &WorkspaceConfig) -> Result<()> {
    bootstrap::ensure_default_agent_skill_file()?;

    let repository = WorkspaceRepository::open(INDEX_FILE)
        .context("failed to open workspace database; run `memento init` first")?;
    repository
        .initialize_schema()
        .context("failed to initialize workspace schema")?;

    for (namespace, root_dir) in [
        (Namespace::Agent, Path::new(AGENT_DIR)),
        (Namespace::User, Path::new(USER_DIR)),
    ] {
        for item_path in collect_item_files(root_dir)? {
            let relative_item_path = item_path
                .strip_prefix(root_dir)
                .expect("indexed path should be inside namespace dir");
            let uri_path = namespace_uri_path(relative_item_path)?;
            let uri = crate::uri::build_namespace_item_uri(namespace, &uri_path);
            let source_path = normalize_source_path(&item_path)?;

            if let Some(existing_item) = repository.get_item_by_source_path(&source_path)? {
                if existing_item.uri == uri {
                    continue;
                }

                repository.delete_item(existing_item.id)?;
            } else if repository.get_item_by_uri(&uri)?.is_some() {
                continue;
            }

            println!(
                "memento serve auto-indexing {} item {} from {}",
                namespace.as_str(),
                uri,
                source_path
            );
            index_namespace_item(&repository, config, namespace, &uri_path, &source_path)?;
        }
    }

    Ok(())
}

fn collect_item_files(path: &Path) -> Result<Vec<std::path::PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();

    for entry in fs::read_dir(path)
        .with_context(|| format!("failed to read namespace directory `{}`", path.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", path.display()))?;
        let entry_path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect `{}`", entry_path.display()))?;

        if file_type.is_dir() {
            files.extend(collect_item_files(&entry_path)?);
        } else if file_type.is_file() {
            files.push(entry_path);
        }
    }

    files.sort();
    Ok(files)
}

fn namespace_uri_path(path: &Path) -> Result<String> {
    let normalized = normalize_source_path(path)?;
    Ok(normalized
        .strip_prefix('/')
        .unwrap_or(&normalized)
        .to_string())
}

fn normalize_source_path(path: &Path) -> Result<String> {
    Ok(path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/"))
}

fn handle_execute(
    request: &mut tiny_http::Request,
    write_lock: &Mutex<()>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let request_start = Instant::now();
    let mut body = String::new();

    if let Err(error) = request.as_reader().read_to_string(&mut body) {
        return json_error(
            StatusCode(400),
            &format!("failed to read request body: {error}"),
        );
    }

    let read_body_elapsed = request_start.elapsed();

    let payload: ExecuteRequest = match serde_json::from_str(&body) {
        Ok(payload) => payload,
        Err(error) => {
            return json_error(
                StatusCode(400),
                &format!("failed to parse request body: {error}"),
            );
        }
    };

    let parse_elapsed = request_start.elapsed();
    let command_name = command_name(&payload.command);

    if timing_enabled() {
        eprintln!(
            "[memento:debug] command={command_name} phase=request_read latency_ms={}",
            read_body_elapsed.as_millis()
        );
        eprintln!(
            "[memento:debug] command={command_name} phase=request_parse latency_ms={}",
            parse_elapsed.as_millis()
        );
    }

    let result: anyhow::Result<crate::protocol::ExecuteResponse> = match &payload.command {
        RemoteCommand::Add { .. }
        | RemoteCommand::Remember { .. }
        | RemoteCommand::Reindex { .. } => {
            let lock_start = Instant::now();
            let _guard = match write_lock.lock() {
                Ok(guard) => guard,
                Err(_) => return json_error(StatusCode(500), "write lock poisoned"),
            };
            if timing_enabled() {
                eprintln!(
                    "[memento:debug] command={command_name} phase=write_lock_wait latency_ms={}",
                    lock_start.elapsed().as_millis()
                );
            }

            let execute_start = Instant::now();
            let result = dispatch::execute_remote_structured(payload.command);
            if timing_enabled() {
                eprintln!(
                    "[memento:debug] command={command_name} phase=execute latency_ms={}",
                    execute_start.elapsed().as_millis()
                );
            }
            result
        }
        _ => {
            let execute_start = Instant::now();
            let result = dispatch::execute_remote_structured(payload.command);
            if timing_enabled() {
                eprintln!(
                    "[memento:debug] command={command_name} phase=execute latency_ms={}",
                    execute_start.elapsed().as_millis()
                );
            }
            result
        }
    };

    if timing_enabled() {
        let status = if result.is_ok() { "ok" } else { "error" };
        eprintln!(
            "[memento:debug] command={command_name} status={status} total_latency_ms={}",
            request_start.elapsed().as_millis()
        );
    }

    match result {
        Ok(output) => json_response(StatusCode(200), &output),
        Err(error) => json_error(StatusCode(500), &error.to_string()),
    }
}

fn json_response<T: serde::Serialize>(
    status: StatusCode,
    payload: &T,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_vec(payload)
        .unwrap_or_else(|_| b"{\"error\":\"serialization failed\"}".to_vec());
    let mut response = Response::from_data(body).with_status_code(status);

    if let Ok(header) = Header::from_bytes("Content-Type", "application/json") {
        response.add_header(header);
    }

    response
}

fn json_error(status: StatusCode, message: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        status,
        &ErrorResponse {
            error: message.to_string(),
        },
    )
}

fn command_name(command: &RemoteCommand) -> &'static str {
    match command {
        RemoteCommand::Doctor => "doctor",
        RemoteCommand::Models => "models",
        RemoteCommand::Add { .. } => "add",
        RemoteCommand::Rm { .. } => "rm",
        RemoteCommand::Remember { .. } => "remember",
        RemoteCommand::Reindex { .. } => "reindex",
        RemoteCommand::Forget { .. } => "forget",
        RemoteCommand::Ls { .. } => "ls",
        RemoteCommand::Cat { .. } => "cat",
        RemoteCommand::Show { .. } => "show",
        RemoteCommand::Find { .. } => "find",
    }
}
