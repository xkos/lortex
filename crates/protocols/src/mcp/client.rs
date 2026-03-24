//! MCP Client — connects to an MCP server and exposes its tools.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info};

use lortex_core::error::ToolError;
use lortex_core::tool::{Tool, ToolContext, ToolOutput};

use super::types::*;

/// MCP client that connects to an MCP server.
pub struct McpClient {
    transport: McpTransport,
    server_capabilities: Option<McpServerCapabilities>,
}

impl McpClient {
    /// Create a new MCP client with the given transport.
    pub fn new(transport: McpTransport) -> Self {
        Self {
            transport,
            server_capabilities: None,
        }
    }

    /// Create a client connecting via stdio to a local server.
    pub fn stdio(command: impl Into<String>, args: Vec<String>) -> Self {
        Self::new(McpTransport::Stdio {
            command: command.into(),
            args,
        })
    }

    /// Create a client connecting via SSE to a remote server.
    pub fn sse(url: impl Into<String>) -> Self {
        Self::new(McpTransport::Sse { url: url.into() })
    }

    /// Initialize the MCP connection and negotiate capabilities.
    pub async fn initialize(&mut self) -> Result<McpServerCapabilities, String> {
        // Send initialize request
        let request = JsonRpcRequest::new(
            Value::Number(1.into()),
            "initialize",
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "lortex",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        );

        let response = self.send_request(request).await?;

        let capabilities = response
            .result
            .and_then(|r| {
                serde_json::from_value::<McpServerCapabilities>(
                    r.get("capabilities").cloned().unwrap_or(Value::Null),
                )
                .ok()
            })
            .unwrap_or_default();

        self.server_capabilities = Some(capabilities.clone());

        // Send initialized notification
        let _ = self
            .send_notification("notifications/initialized", Value::Null)
            .await;

        info!("MCP client initialized");
        Ok(capabilities)
    }

    /// Discover tools offered by the MCP server.
    pub async fn discover_tools(&self) -> Result<Vec<Arc<dyn Tool>>, String> {
        let request = JsonRpcRequest::new(
            Value::Number(2.into()),
            "tools/list",
            Value::Object(Default::default()),
        );

        let response = self.send_request(request).await?;

        let tools_value = response
            .result
            .and_then(|r| r.get("tools").cloned())
            .unwrap_or(Value::Array(vec![]));

        let tool_defs: Vec<McpToolDefinition> =
            serde_json::from_value(tools_value).map_err(|e| e.to_string())?;

        let tools: Vec<Arc<dyn Tool>> = tool_defs
            .into_iter()
            .map(|def| {
                Arc::new(McpRemoteTool {
                    name: def.name,
                    description: def.description,
                    schema: def.input_schema,
                    transport: self.transport.clone(),
                }) as Arc<dyn Tool>
            })
            .collect();

        info!(count = tools.len(), "Discovered MCP tools");
        Ok(tools)
    }

    /// Get a resource from the MCP server.
    pub async fn get_resource(&self, uri: &str) -> Result<McpResourceContents, String> {
        let request = JsonRpcRequest::new(
            Value::Number(3.into()),
            "resources/read",
            serde_json::json!({ "uri": uri }),
        );

        let response = self.send_request(request).await?;

        let contents = response
            .result
            .and_then(|r| r.get("contents").and_then(|c| c.get(0)).cloned())
            .ok_or("No contents in response")?;

        serde_json::from_value(contents).map_err(|e| e.to_string())
    }

    /// Send a JSON-RPC request to the server.
    async fn send_request(&self, request: JsonRpcRequest) -> Result<JsonRpcResponse, String> {
        match &self.transport {
            McpTransport::Sse { url } => {
                let client = reqwest::Client::new();
                let resp = client
                    .post(url)
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| format!("HTTP request failed: {}", e))?;

                resp.json::<JsonRpcResponse>()
                    .await
                    .map_err(|e| format!("Failed to parse response: {}", e))
            }
            McpTransport::Stdio { command: _, args: _ } => {
                // For stdio transport, we'd need to spawn the process and communicate
                // via stdin/stdout. This is a simplified placeholder.
                Err("Stdio transport not yet fully implemented. Use SSE transport.".into())
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&self, method: &str, _params: Value) -> Result<(), String> {
        // Notifications don't expect a response
        debug!(method = method, "Sending MCP notification");
        Ok(())
    }
}

/// A tool backed by a remote MCP server.
struct McpRemoteTool {
    name: String,
    description: String,
    schema: Value,
    transport: McpTransport,
}

#[async_trait]
impl Tool for McpRemoteTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters_schema(&self) -> Value {
        self.schema.clone()
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let request = JsonRpcRequest::new(
            Value::Number(1.into()),
            "tools/call",
            serde_json::json!({
                "name": self.name,
                "arguments": args,
            }),
        );

        match &self.transport {
            McpTransport::Sse { url } => {
                let client = reqwest::Client::new();
                let resp = client
                    .post(url)
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("MCP call failed: {}", e)))?;

                let rpc_response: JsonRpcResponse = resp
                    .json()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Parse error: {}", e)))?;

                if let Some(error) = rpc_response.error {
                    return Err(ToolError::ExecutionFailed(error.message));
                }

                let result = rpc_response.result.unwrap_or(Value::Null);

                // Extract text content from MCP response
                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    let texts: Vec<String> = content
                        .iter()
                        .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                        .map(|s| s.to_string())
                        .collect();
                    Ok(ToolOutput::text(texts.join("\n")))
                } else {
                    Ok(ToolOutput::json(result))
                }
            }
            McpTransport::Stdio { .. } => Err(ToolError::ExecutionFailed(
                "Stdio transport not yet fully implemented".into(),
            )),
        }
    }
}
