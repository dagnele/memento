use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::bootstrap;
use crate::config::WorkspaceConfig;
use crate::dispatch;
use crate::indexing::index_namespace_item;
use crate::protocol::{ErrorResponse, ExecuteRequest, ExecuteResponse, RemoteCommand};
use crate::repository::workspace::{AGENT_DIR, INDEX_FILE, USER_DIR, WorkspaceRepository};
use crate::timing::{enable_timing, timing_enabled};
use crate::uri::Namespace;

struct AppState {
    write_lock: Mutex<()>,
    debug: bool,
}

pub fn serve(debug: bool) -> Result<()> {
    if debug {
        enable_timing();
    }

    let config = WorkspaceConfig::load()
        .context("failed to load workspace config; run `memento init` first")?;
    let address = format!("127.0.0.1:{}", config.server_port);

    ensure_namespace_items_indexed(&config)?;

    let state = Arc::new(AppState {
        write_lock: Mutex::new(()),
        debug,
    });

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    runtime.block_on(async {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/v1/execute", post(execute_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .with_context(|| format!("failed to bind server on {address}"))?;

        println!("memento serve listening on http://{address}");

        if debug {
            eprintln!("[memento:debug] server debug logging enabled");
        }

        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        });

        if let Err(error) = server.await {
            eprintln!("server exited with error: {error}");
        }

        Ok(())
    })
}

pub fn ensure_namespace_items_indexed(config: &WorkspaceConfig) -> Result<()> {
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

            eprintln!(
                "memento auto-indexing {} item {} from {}",
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

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn execute_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ExecuteRequest>,
) -> impl IntoResponse {
    let request_start = Instant::now();
    let command_name = command_name(&payload.command);
    let debug = state.debug;

    let result: Result<ExecuteResponse, String> = tokio::task::spawn_blocking(move || {
        let result: anyhow::Result<ExecuteResponse> = match &payload.command {
            RemoteCommand::Add { .. }
            | RemoteCommand::Remember { .. }
            | RemoteCommand::Reindex { .. } => {
                let lock_start = Instant::now();
                let _guard = match state.write_lock.lock() {
                    Ok(guard) => guard,
                    Err(_) => return Err("write lock poisoned".to_string()),
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

        match result {
            Ok(response) => Ok(response),
            Err(error) => Err(error.to_string()),
        }
    })
    .await
    .unwrap_or_else(|error| Err(format!("task panicked: {error}")));

    if debug {
        let status = if result.is_ok() { "ok" } else { "error" };
        eprintln!(
            "[memento:debug] command={command_name} status={status} total_latency_ms={}",
            request_start.elapsed().as_millis()
        );
    }

    match result {
        Ok(response) => (
            StatusCode::OK,
            Json(serde_json::to_value(&response).unwrap()),
        )
            .into_response(),
        Err(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::to_value(&ErrorResponse { error: message }).unwrap()),
        )
            .into_response(),
    }
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
