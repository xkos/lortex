//! Tool approval guardrail — requires human approval for specified tools.

use async_trait::async_trait;

use lortex_core::guardrail::{Guardrail, GuardrailResult};
use lortex_core::message::{ContentPart, Message};

/// A guardrail that blocks tool calls requiring human approval.
///
/// When a tool call in the output matches one of the `require_approval` tools,
/// this guardrail blocks execution until approval is granted.
pub struct ToolApproval {
    /// Tool names that require human approval before execution.
    require_approval: Vec<String>,
}

impl ToolApproval {
    /// Create a new tool approval guardrail.
    pub fn new(tools: Vec<String>) -> Self {
        Self {
            require_approval: tools,
        }
    }

    /// Add a tool that requires approval.
    pub fn add_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.require_approval.push(tool_name.into());
        self
    }
}

#[async_trait]
impl Guardrail for ToolApproval {
    fn name(&self) -> &str {
        "tool_approval"
    }

    async fn check_input(&self, _messages: &[Message]) -> GuardrailResult {
        // Input guardrail: pass (tool approval only applies to output)
        GuardrailResult::Pass
    }

    async fn check_output(&self, output: &Message) -> GuardrailResult {
        for part in &output.content {
            if let ContentPart::ToolCall { name, .. } = part {
                if self.require_approval.contains(name) {
                    return GuardrailResult::Block {
                        message: format!(
                            "Tool '{}' requires human approval before execution",
                            name
                        ),
                    };
                }
            }
        }
        GuardrailResult::Pass
    }
}
