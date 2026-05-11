//! File read tool.

use async_trait::async_trait;
use nexus_core::tool::Tool;
use nexus_core::types::{ToolContext, ToolResult, ToolSchema};
use std::path::Path;

pub struct FileReadTool {
    schema: ToolSchema,
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            schema: ToolSchema {
                name: "file_read".to_string(),
                description: "Read the contents of a file. Returns the content with line numbers."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file_path"],
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to read (relative to project root)"
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Line number to start from (0-based)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of lines to return (default: 2000)"
                        }
                    }
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };

        let offset = input.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000) as usize;

        let resolved = context.project_root.join(file_path);

        // Safety: check the path is within project root
        let canonical_resolved = match resolved.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                // File might not exist yet, but check parent directory exists
                if !resolved.exists() {
                    return ToolResult::err(format!("File not found: {}", file_path));
                }
                resolved.clone()
            }
        };

        // Canonicalize both paths for comparison to handle symlinks and different representations
        let canonical_root = match context.project_root.canonicalize() {
            Ok(path) => path,
            Err(_) => context.project_root.clone(),
        };

        if !canonical_resolved.starts_with(&canonical_root) {
            return ToolResult::err("Path is outside the project root");
        }

        // Read the file
        match tokio::fs::read_to_string(&resolved).await {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();
                let end = (offset + limit).min(total_lines);
                let selected = &lines[offset.min(total_lines)..end];

                let numbered: String = selected
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{}\t{}", offset + i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n");

                let truncated = end < total_lines;
                ToolResult::ok(numbered)
                    .with_meta("path", serde_json::json!(file_path))
                    .with_meta("line_count", serde_json::json!(total_lines))
                    .with_meta("truncated", serde_json::json!(truncated))
            }
            Err(e) => ToolResult::err(format!("Failed to read file: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn test_context(dir: &Path) -> ToolContext {
        ToolContext {
            project_root: dir.to_path_buf(),
            cwd: dir.to_path_buf(),
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn reads_existing_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();

        let tool = FileReadTool::new();
        let result = tool
            .execute(
                serde_json::json!({"file_path": "test.txt"}),
                &test_context(dir.path()),
            )
            .await;

        assert!(result.success);
        assert!(result.output.contains("line 1"));
        assert!(result.output.contains("line 2"));
    }

    #[tokio::test]
    async fn returns_error_for_missing_file() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new();
        let result = tool
            .execute(
                serde_json::json!({"file_path": "nonexistent.txt"}),
                &test_context(dir.path()),
            )
            .await;

        assert!(!result.success);
    }
}
