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

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::guardrail::Guardrail;
    use lortex_core::message::{ContentPart, Message};
    use serde_json::json;

    fn make_tool_call_message(tool_name: &str) -> Message {
        let mut msg = Message::assistant("");
        msg.content = vec![ContentPart::ToolCall {
            id: "call_1".into(),
            name: tool_name.into(),
            arguments: json!({}),
        }];
        msg
    }

    #[test]
    fn name_returns_tool_approval() {
        let guard = ToolApproval::new(vec![]);
        assert_eq!(guard.name(), "tool_approval");
    }

    #[tokio::test]
    async fn check_input_always_passes() {
        let guard = ToolApproval::new(vec!["dangerous".into()]);
        let result = guard.check_input(&[Message::user("anything")]).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn blocks_restricted_tool() {
        let guard = ToolApproval::new(vec!["shell".into()]);
        let msg = make_tool_call_message("shell");
        let result = guard.check_output(&msg).await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn passes_unrestricted_tool() {
        let guard = ToolApproval::new(vec!["shell".into()]);
        let msg = make_tool_call_message("read_file");
        let result = guard.check_output(&msg).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn passes_text_only_output() {
        let guard = ToolApproval::new(vec!["shell".into()]);
        let result = guard
            .check_output(&Message::assistant("just text"))
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn blocks_first_restricted_in_multi_tool() {
        let guard = ToolApproval::new(vec!["shell".into()]);
        let mut msg = Message::assistant("");
        msg.content = vec![
            ContentPart::ToolCall {
                id: "c1".into(),
                name: "read_file".into(),
                arguments: json!({}),
            },
            ContentPart::ToolCall {
                id: "c2".into(),
                name: "shell".into(),
                arguments: json!({}),
            },
        ];
        let result = guard.check_output(&msg).await;
        assert!(result.is_block());
    }

    #[test]
    fn add_tool_builder() {
        let guard = ToolApproval::new(vec![]).add_tool("shell").add_tool("delete");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(guard.check_output(&make_tool_call_message("delete")));
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn empty_approval_list_passes_all() {
        let guard = ToolApproval::new(vec![]);
        let msg = make_tool_call_message("anything");
        let result = guard.check_output(&msg).await;
        assert!(result.is_pass());
    }
}
