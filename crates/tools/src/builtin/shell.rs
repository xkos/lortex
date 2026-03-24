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
