//! File write tool.

use async_trait::async_trait;
use nexus_core::tool::Tool;
use nexus_core::types::{ToolContext, ToolResult, ToolSchema};

pub struct FileWriteTool {
    schema: ToolSchema,
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self {
            schema: ToolSchema {
                name: "file_write".to_string(),
                description: "Create a new file or overwrite an existing file.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file_path", "content"],
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to create/overwrite"
                        },
                        "content": {
                            "type": "string",
                            "description": "Full content to write"
                        }
                    }
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };
        let content = match input.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: content"),
        };

        let resolved = context.project_root.join(file_path);

        // Create parent directories
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::err(format!("Failed to create directories: {}", e));
            }
        }

        let created = !resolved.exists();
        let byte_size = content.len();

        match tokio::fs::write(&resolved, content).await {
            Ok(_) => ToolResult::ok(format!(
                "{} {} ({} bytes)",
                if created { "Created" } else { "Wrote" },
                file_path,
                byte_size
            ))
            .with_meta("path", serde_json::json!(file_path))
            .with_meta("byte_size", serde_json::json!(byte_size))
            .with_meta("created", serde_json::json!(created)),
            Err(e) => ToolResult::err(format!("Failed to write file: {}", e)),
        }
    }
}
