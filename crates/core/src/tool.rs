//! Tool trait and related types.
//!
//! Tools are external capabilities that agents can invoke. Each tool has a name,
//! description, parameter schema (JSON Schema), and an async execute method.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;

use crate::error::ToolError;
use crate::message::Message;

/// Context provided to a tool during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// The current session ID.
    pub session_id: String,

    /// The agent that invoked the tool.
    pub agent_name: String,

    /// Recent conversation messages for context.
    pub messages: Vec<Message>,
}

/// Output returned from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The result content (can be any JSON value).
    pub content: Value,

    /// Whether this result represents an error.
    pub is_error: bool,
}

impl ToolOutput {
    /// Create a successful text output.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: Value::String(text.into()),
            is_error: false,
        }
    }

    /// Create a successful JSON output.
    pub fn json(value: Value) -> Self {
        Self {
            content: value,
            is_error: false,
        }
    }

    /// Create an error output.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: Value::String(message.into()),
            is_error: true,
        }
    }
}

/// An example of tool usage, provided to improve LLM calling accuracy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExample {
    /// Description of what this example demonstrates.
    pub description: String,

    /// Example input arguments.
    pub input: Value,

    /// Example output.
    pub output: Value,
}

/// The core Tool trait. Any external capability that an agent can invoke
/// must implement this trait.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of the tool (used by LLMs to invoke it).
    fn name(&self) -> &str;

    /// A detailed description of the tool (3-4 sentences recommended).
    /// This description helps the LLM decide when and how to use the tool.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    fn parameters_schema(&self) -> Value;

    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError>;

    /// Optional examples that help the LLM understand how to call the tool.
    fn examples(&self) -> Vec<ToolExample> {
        vec![]
    }

    /// Whether this tool requires human approval before execution.
    fn requires_approval(&self) -> bool {
        false
    }
}

impl fmt::Debug for dyn Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name())
            .field("description", &self.description())
            .finish()
    }
}

/// Type alias for boxed async tool functions.
pub type BoxToolFn = Arc<
    dyn Fn(Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ToolOutput, ToolError>> + Send>>
        + Send
        + Sync,
>;

/// A convenience wrapper that converts a closure into a Tool.
///
/// # Example
/// ```rust,ignore
/// let tool = FnTool::new(
///     "greet",
///     "Greet someone by name",
///     serde_json::json!({"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}),
///     |args| Box::pin(async move {
///         let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("world");
///         Ok(ToolOutput::text(format!("Hello, {}!", name)))
///     }),
/// );
/// ```
pub struct FnTool {
    pub name: String,
    pub description: String,
    pub schema: Value,
    pub func: BoxToolFn,
}

impl FnTool {
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: Value,
        func: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ToolOutput, ToolError>> + Send + 'static,
    {
        let func: BoxToolFn = Arc::new(move |args: Value| {
            Box::pin(func(args))
                as std::pin::Pin<
                    Box<dyn std::future::Future<Output = Result<ToolOutput, ToolError>> + Send>,
                >
        });

        Self {
            name: name.into(),
            description: description.into(),
            schema,
            func,
        }
    }
}

#[async_trait]
impl Tool for FnTool {
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
        (self.func)(args).await
    }
}
