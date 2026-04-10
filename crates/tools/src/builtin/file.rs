//! File operation tools — read and write files.

use async_trait::async_trait;
use serde_json::Value;

use lortex_core::error::ToolError;
use lortex_core::tool::{Tool, ToolContext, ToolOutput};

/// Tool for reading file contents.
pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Use this tool when you need to \
         examine the contents of an existing file. Returns the file contents as a string. \
         Fails if the file does not exist or cannot be read."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to the file to read."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' argument".into()))?;

        match tokio::fs::read_to_string(path).await {
            Ok(content) => Ok(ToolOutput::text(content)),
            Err(e) => Err(ToolError::ExecutionFailed(format!(
                "Failed to read file '{}': {}",
                path, e
            ))),
        }
    }
}

/// Tool for writing content to a file.
pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates the file if it doesn't exist, \
         or overwrites it if it does. Use this when you need to create or modify a file. \
         Parent directories must already exist."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The absolute or relative path to the file to write."
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'path' argument".into()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("Missing 'content' argument".into()))?;

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to create directory: {}", e))
                })?;
            }
        }

        match tokio::fs::write(path, content).await {
            Ok(()) => Ok(ToolOutput::text(format!(
                "Successfully wrote {} bytes to '{}'",
                content.len(),
                path
            ))),
            Err(e) => Err(ToolError::ExecutionFailed(format!(
                "Failed to write file '{}': {}",
                path, e
            ))),
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
    use std::io::Write;

    fn test_ctx() -> ToolContext {
        ToolContext {
            session_id: "test".into(),
            agent_name: "test".into(),
            messages: vec![],
        }
    }

    // --- ReadFileTool ---

    #[test]
    fn read_file_tool_metadata() {
        let tool = ReadFileTool;
        assert_eq!(tool.name(), "read_file");
        assert!(!tool.requires_approval());
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"][0], "path");
    }

    #[tokio::test]
    async fn read_file_success() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp, "hello from file").unwrap();

        let result = ReadFileTool
            .execute(json!({"path": tmp.path().to_str().unwrap()}), &test_ctx())
            .await
            .unwrap();
        assert_eq!(result.content.as_str().unwrap(), "hello from file");
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn read_file_missing_path_arg() {
        let result = ReadFileTool.execute(json!({}), &test_ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_file_nonexistent() {
        let result = ReadFileTool
            .execute(json!({"path": "/tmp/lortex_nonexistent_file_xyz"}), &test_ctx())
            .await;
        assert!(result.is_err());
    }

    // --- WriteFileTool ---

    #[test]
    fn write_file_tool_metadata() {
        let tool = WriteFileTool;
        assert_eq!(tool.name(), "write_file");
        assert!(tool.requires_approval());
        let schema = tool.parameters_schema();
        assert!(schema["required"].as_array().unwrap().contains(&json!("path")));
        assert!(schema["required"].as_array().unwrap().contains(&json!("content")));
    }

    #[tokio::test]
    async fn write_file_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");

        let result = WriteFileTool
            .execute(
                json!({"path": path.to_str().unwrap(), "content": "written content"}),
                &test_ctx(),
            )
            .await
            .unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "written content");
    }

    #[tokio::test]
    async fn write_file_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("deep").join("file.txt");

        let result = WriteFileTool
            .execute(
                json!({"path": path.to_str().unwrap(), "content": "nested"}),
                &test_ctx(),
            )
            .await
            .unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "nested");
    }

    #[tokio::test]
    async fn write_file_missing_args() {
        let r1 = WriteFileTool.execute(json!({}), &test_ctx()).await;
        assert!(r1.is_err());

        let r2 = WriteFileTool
            .execute(json!({"path": "/tmp/x"}), &test_ctx())
            .await;
        assert!(r2.is_err());
    }
}
