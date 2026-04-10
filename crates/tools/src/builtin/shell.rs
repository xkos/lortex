//! Shell tool — execute shell commands.

use async_trait::async_trait;
use serde_json::Value;

use lortex_core::error::ToolError;
use lortex_core::tool::{Tool, ToolContext, ToolOutput};

/// Tool for executing shell commands.
pub struct ShellTool {
    /// Optional working directory for command execution.
    pub working_dir: Option<String>,

    /// Maximum execution time in seconds.
    pub timeout_secs: u64,
}

impl ShellTool {
    pub fn new() -> Self {
        Self {
            working_dir: None,
            timeout_secs: 30,
        }
    }

    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output. Use this tool to run system \
         commands, scripts, or CLI tools. Returns both stdout and stderr. The command \
         is executed in a shell (sh on Unix, cmd on Windows)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command."
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'command' argument".into()))?;

        let working_dir = args
            .get("working_dir")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| self.working_dir.clone());

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", command]);
            c
        };

        if let Some(dir) = &working_dir {
            cmd.current_dir(dir);
        }

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| ToolError::Timeout(self.timeout_secs * 1000))?
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let exit_code = result.status.code().unwrap_or(-1);

        let output = format!(
            "Exit code: {}\n\nStdout:\n{}\n\nStderr:\n{}",
            exit_code, stdout, stderr
        );

        if result.status.success() {
            Ok(ToolOutput::text(output))
        } else {
            Ok(ToolOutput {
                content: Value::String(output),
                is_error: true,
            })
        }
    }

    fn requires_approval(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::tool::Tool;
    use serde_json::json;

    fn test_ctx() -> ToolContext {
        ToolContext {
            session_id: "test".into(),
            agent_name: "test".into(),
            messages: vec![],
        }
    }

    #[test]
    fn shell_tool_metadata() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "shell");
        assert!(tool.requires_approval());
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "command");
    }

    #[test]
    fn default_impl() {
        let tool = ShellTool::default();
        assert_eq!(tool.timeout_secs, 30);
        assert!(tool.working_dir.is_none());
    }

    #[test]
    fn builder_methods() {
        let tool = ShellTool::new().with_working_dir("/tmp").with_timeout(60);
        assert_eq!(tool.working_dir.as_deref(), Some("/tmp"));
        assert_eq!(tool.timeout_secs, 60);
    }

    #[tokio::test]
    async fn executes_simple_command() {
        let tool = ShellTool::new();
        let result = tool
            .execute(json!({"command": "echo hello"}), &test_ctx())
            .await
            .unwrap();
        assert!(!result.is_error);
        assert!(result.content.as_str().unwrap().contains("hello"));
    }

    #[tokio::test]
    async fn captures_exit_code_on_failure() {
        let tool = ShellTool::new();
        let result = tool
            .execute(json!({"command": "false"}), &test_ctx())
            .await
            .unwrap();
        assert!(result.is_error);
        assert!(result.content.as_str().unwrap().contains("Exit code:"));
    }

    #[tokio::test]
    async fn missing_command_arg() {
        let tool = ShellTool::new();
        let result = tool.execute(json!({}), &test_ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn respects_working_dir_from_args() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellTool::new();
        let result = tool
            .execute(
                json!({"command": "pwd", "working_dir": dir.path().to_str().unwrap()}),
                &test_ctx(),
            )
            .await
            .unwrap();
        assert!(!result.is_error);
        // The output should contain the temp dir path
        let output = result.content.as_str().unwrap();
        // Resolve symlinks for macOS /private/var vs /var
        let canonical = dir.path().canonicalize().unwrap();
        assert!(output.contains(canonical.to_str().unwrap()));
    }
}
