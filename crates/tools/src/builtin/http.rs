//! HTTP tool — make HTTP requests.

use async_trait::async_trait;
use serde_json::Value;

use lortex_core::error::ToolError;
use lortex_core::tool::{Tool, ToolContext, ToolOutput};

/// Tool for making HTTP requests.
pub struct HttpTool {
    client: reqwest::Client,
    timeout_secs: u64,
}

impl HttpTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            timeout_secs: 30,
        }
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for HttpTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &str {
        "http_request"
    }

    fn description(&self) -> &str {
        "Make an HTTP request to a URL. Supports GET, POST, PUT, DELETE, and PATCH methods. \
         Use this tool to interact with REST APIs, fetch web pages, or send data to services. \
         Returns the response status code, headers, and body."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to make the request to."
                },
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE", "PATCH"],
                    "description": "The HTTP method to use. Defaults to GET."
                },
                "headers": {
                    "type": "object",
                    "description": "Optional HTTP headers as key-value pairs."
                },
                "body": {
                    "type": "string",
                    "description": "Optional request body (for POST, PUT, PATCH)."
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'url' argument".into()))?;

        let method = args
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        let mut request = match method.as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            "PATCH" => self.client.patch(url),
            other => {
                return Err(ToolError::InvalidArguments(format!(
                    "Unsupported HTTP method: {}",
                    other
                )));
            }
        };

        // Add headers
        if let Some(headers) = args.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                if let Some(val_str) = value.as_str() {
                    request = request.header(key.as_str(), val_str);
                }
            }
        }

        // Add body
        if let Some(body) = args.get("body").and_then(|v| v.as_str()) {
            request = request.body(body.to_string());
        }

        // Set timeout
        request = request.timeout(std::time::Duration::from_secs(self.timeout_secs));

        let response = request
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status().as_u16();
        let headers: serde_json::Map<String, Value> = response
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    Value::String(v.to_str().unwrap_or("").to_string()),
                )
            })
            .collect();

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read response: {}", e)))?;

        Ok(ToolOutput::json(serde_json::json!({
            "status": status,
            "headers": headers,
            "body": body,
        })))
    }
}
