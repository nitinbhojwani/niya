//! File edit tool — targeted search-and-replace.

use async_trait::async_trait;
use nexus_core::tool::Tool;
use nexus_core::types::{ToolContext, ToolResult, ToolSchema};

pub struct FileEditTool {
    schema: ToolSchema,
}

impl FileEditTool {
    pub fn new() -> Self {
        Self {
            schema: ToolSchema {
                name: "file_edit".to_string(),
                description: "Make a targeted search-and-replace edit to an existing file."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["file_path", "old_string", "new_string"],
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path to the file to edit"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "Exact text to find"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement text"
                        },
                        "replace_all": {
                            "type": "boolean",
                            "description": "Replace all occurrences (default: false)"
                        }
                    }
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: file_path"),
        };
        let old_string = match input.get("old_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: old_string"),
        };
        let new_string = match input.get("new_string").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::err("Missing required parameter: new_string"),
        };
        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let resolved = context.project_root.join(file_path);

        // Read the file
        let content = match tokio::fs::read_to_string(&resolved).await {
            Ok(c) => c,
            Err(e) => return ToolResult::err(format!("Failed to read file: {}", e)),
        };

        // Count occurrences
        let count = content.matches(old_string).count();

        if count == 0 {
            return ToolResult::err(format!(
                "old_string not found in {}. Make sure the text matches exactly.",
                file_path
            ));
        }

        if count > 1 && !replace_all {
            return ToolResult::err(format!(
                "old_string found {} times in {}. Provide more context for a unique match, or set replace_all: true.",
                count, file_path
            ));
        }

        // Perform the replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        match tokio::fs::write(&resolved, &new_content).await {
            Ok(_) => ToolResult::ok(format!(
                "Edited {}: {} replacement(s) made",
                file_path, count
            ))
            .with_meta("path", serde_json::json!(file_path))
            .with_meta("replacements", serde_json::json!(count)),
            Err(e) => ToolResult::err(format!("Failed to write file: {}", e)),
        }
    }
}
