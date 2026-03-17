use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result, anyhow};
use axum::Router;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, ErrorData, Implementation, ListResourceTemplatesResult,
    ListResourcesResult, RawResourceTemplate, ReadResourceRequestParams, ReadResourceResult,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{RoleServer, ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;

use crate::cli::MemoryNamespace;
use crate::repository::workspace::{INDEX_FILE, WorkspaceRepository};
use crate::service;

#[derive(Debug, Clone)]
pub struct McpServerHandle {
    shutdown: CancellationToken,
}

impl McpServerHandle {
    pub fn shutdown(self) {
        self.shutdown.cancel();
    }
}

pub fn spawn_http_server(port: u16) -> Result<McpServerHandle> {
    let shutdown = CancellationToken::new();
    let child_shutdown = shutdown.child_token();
    let address = SocketAddr::from(([127, 0, 0, 1], port));

    thread::Builder::new()
        .name("memento-mcp".to_string())
        .spawn(move || {
            let runtime = match Builder::new_multi_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("failed to start MCP runtime: {error}");
                    return;
                }
            };

            runtime.block_on(async move {
                let session_manager = Arc::new(LocalSessionManager::default());
                let service: StreamableHttpService<MementoMcpServer, LocalSessionManager> =
                    StreamableHttpService::new(
                        || Ok(MementoMcpServer::new()),
                        session_manager,
                        StreamableHttpServerConfig {
                            stateful_mode: false,
                            json_response: true,
                            cancellation_token: child_shutdown.clone(),
                            ..Default::default()
                        },
                    );

                let app = Router::new().fallback_service(service);

                let listener = match tokio::net::TcpListener::bind(address).await {
                    Ok(listener) => listener,
                    Err(error) => {
                        eprintln!("failed to bind MCP server on {address}: {error}");
                        return;
                    }
                };

                let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                    child_shutdown.cancelled_owned().await;
                });

                if let Err(error) = server.await {
                    eprintln!("MCP server exited with error: {error}");
                }
            });
        })
        .map_err(|error| anyhow!(error.to_string()))
        .with_context(|| format!("failed to spawn MCP server thread for {address}"))?;

    Ok(McpServerHandle { shutdown })
}

#[derive(Debug, Clone)]
struct MementoMcpServer {
    tool_router: ToolRouter<Self>,
}

impl MementoMcpServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    #[serde(default)]
    force: bool,
    paths: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RmArgs {
    target: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RememberArgs {
    namespace: MemoryNamespace,
    path: String,
    file: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReindexArgs {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ForgetArgs {
    uri: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LsArgs {
    uri: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct UriArgs {
    uri: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FindArgs {
    query: String,
}

#[tool_router]
impl MementoMcpServer {
    #[tool(
        name = "doctor",
        description = "Inspect workspace health and configuration"
    )]
    async fn doctor(&self) -> Result<CallToolResult, ErrorData> {
        let result = service::doctor::execute().map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "models", description = "List supported embedding models")]
    async fn models(&self) -> Result<CallToolResult, ErrorData> {
        let result = service::models::execute().map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "add", description = "Index resource files into the workspace")]
    async fn add(
        &self,
        Parameters(args): Parameters<AddArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::add::execute(args.force, args.paths).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "rm", description = "Untrack a previously indexed resource")]
    async fn rm(&self, Parameters(args): Parameters<RmArgs>) -> Result<CallToolResult, ErrorData> {
        let result = service::rm::execute(args.target).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "remember", description = "Store a user or agent memory item")]
    async fn remember(
        &self,
        Parameters(args): Parameters<RememberArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::remember::execute(args.namespace, args.path, args.file, args.text)
            .map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "reindex", description = "Refresh indexed resources from disk")]
    async fn reindex(
        &self,
        Parameters(args): Parameters<ReindexArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::reindex::execute(args.paths).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "forget", description = "Remove a stored memory item")]
    async fn forget(
        &self,
        Parameters(args): Parameters<ForgetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::forget::execute(args.uri).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "ls", description = "List workspace resources or memory items")]
    async fn ls(&self, Parameters(args): Parameters<LsArgs>) -> Result<CallToolResult, ErrorData> {
        let result = service::ls::execute(args.uri).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "cat", description = "Read a resource or memory item contents")]
    async fn cat(
        &self,
        Parameters(args): Parameters<UriArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::cat::execute(args.uri).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(
        name = "show",
        description = "Show metadata for a resource or memory item"
    )]
    async fn show(
        &self,
        Parameters(args): Parameters<UriArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::show::execute(args.uri).map_err(internal_tool_error)?;
        json_result(result)
    }

    #[tool(name = "find", description = "Search indexed workspace content")]
    async fn find(
        &self,
        Parameters(args): Parameters<FindArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = service::find::execute(args.query).map_err(internal_tool_error)?;
        json_result(result)
    }
}

#[tool_handler]
impl ServerHandler for MementoMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
            .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_06_18)
            .with_server_info(
                Implementation::new("memento", env!("CARGO_PKG_VERSION"))
                    .with_title("Memento MCP")
                    .with_description("Expose Memento workspace commands as MCP tools"),
            )
            .with_instructions(
                "Use these tools to manage and query the local Memento workspace. All operations run against the current working directory's .memento workspace.",
            )
    }

    async fn initialize(
        &self,
        _request: rmcp::model::InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::InitializeResult, ErrorData> {
        Ok(self.get_info())
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let repository = WorkspaceRepository::open(INDEX_FILE).map_err(internal_tool_error)?;
        repository
            .initialize_schema()
            .map_err(internal_tool_error)?;

        let resources = repository
            .list_items()
            .map_err(internal_tool_error)?
            .into_iter()
            .map(resource_from_item)
            .collect();

        Ok(ListResourcesResult::with_all_items(resources))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let result = service::cat::execute(request.uri.clone()).map_err(resource_read_error)?;

        Ok(ReadResourceResult::new(vec![
            ResourceContents::text(result.content, request.uri).with_mime_type("text/plain"),
        ]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult::with_all_items(vec![
            resource_template(
                "mem://resources/{path}",
                "resources",
                "Indexed workspace resources",
            ),
            resource_template("mem://user/{path}", "user", "User memory entries"),
            resource_template("mem://agent/{path}", "agent", "Agent memory entries"),
        ]))
    }
}

fn json_result<T: serde::Serialize>(value: T) -> Result<CallToolResult, ErrorData> {
    let payload = serde_json::to_value(&value).map_err(|error| {
        ErrorData::internal_error(
            "failed to serialize tool response",
            Some(json!({ "reason": error.to_string() })),
        )
    })?;

    Ok(CallToolResult::success(vec![Content::json(payload)?]))
}

fn internal_tool_error(error: anyhow::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

fn resource_read_error(error: anyhow::Error) -> ErrorData {
    ErrorData::resource_not_found(error.to_string(), None)
}

fn resource_from_item(item: crate::repository::workspace::ItemRecord) -> rmcp::model::Resource {
    let name = item
        .uri
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or(item.uri.as_str())
        .to_string();

    let mut description_parts = Vec::new();
    if let Some(source_path) = item.source_path.clone() {
        description_parts.push(format!("source: {source_path}"));
    }
    description_parts.push(format!("namespace: {}", item.namespace));

    let mut resource = rmcp::model::RawResource::new(item.uri, name).with_mime_type("text/plain");
    if !description_parts.is_empty() {
        resource = resource.with_description(description_parts.join("; "));
    }

    resource.no_annotation()
}

fn resource_template(
    uri_template: &str,
    name: &str,
    description: &str,
) -> rmcp::model::ResourceTemplate {
    RawResourceTemplate::new(uri_template, name)
        .with_description(description)
        .with_mime_type("text/plain")
        .no_annotation()
}
