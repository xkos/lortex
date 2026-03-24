//! MCP Server — exposes local tools and resources as an MCP service.

use std::sync::Arc;

use serde_json::Value;
use tracing::info;

use lortex_core::tool::{Tool, ToolContext};

use super::types::*;

/// MCP server that exposes local tools as an MCP service.
pub struct McpServer {
    /// Tools to expose.
    tools: Vec<Arc<dyn Tool>>,

    /// Resources to expose.
    resources: Vec<McpResource>,

    /// Server name.
    name: String,

    /// Server version.
    version: String,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            tools: vec![],
            resources: vec![],
            name: name.into(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Add a tool to the server.
    pub fn add_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add tools to the server.
    pub fn add_tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Add a resource to the server.
    pub fn add_resource(mut self, resource: McpResource) -> Self {
        self.resources.push(resource);
        self
    }

    /// Handle a JSON-RPC request and return a response.
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request),
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                    data: None,
                }),
            },
        }
    }

    fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!(server = %self.name, "MCP server initialized");
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {},
                },
                "serverInfo": {
                    "name": self.name,
                    "version": self.version,
                }
            })),
            error: None,
        }
    }

    fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "inputSchema": t.parameters_schema(),
                })
            })
            .collect();

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::json!({ "tools": tools })),
            error: None,
        }
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let name = request
            .params
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let arguments = request
            .params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Default::default()));

        let tool = self.tools.iter().find(|t| t.name() == name);

        match tool {
            Some(tool) => {
                let ctx = ToolContext {
                    session_id: "mcp".to_string(),
                    agent_name: "mcp-server".to_string(),
                    messages: vec![],
                };

                match tool.execute(arguments, &ctx).await {
                    Ok(output) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": output.content.to_string(),
                            }],
                            "isError": output.is_error,
                        })),
                        error: None,
                    },
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: Some(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": e.to_string(),
                            }],
                            "isError": true,
                        })),
                        error: None,
                    },
                }
            }
            None => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: format!("Tool not found: {}", name),
                    data: None,
                }),
            },
        }
    }

    fn handle_resources_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let resources: Vec<Value> = self
            .resources
            .iter()
            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
            .collect();

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(serde_json::json!({ "resources": resources })),
            error: None,
        }
    }
}
