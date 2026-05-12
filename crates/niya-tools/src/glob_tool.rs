//! Glob tool — find files by pattern.

use async_trait::async_trait;
use niya_core::tool::Tool;
use niya_core::types::{ToolContext, ToolResult, ToolSchema};

pub struct GlobTool {
    schema: ToolSchema,
}

impl GlobTool {
    pub fn new() -> Self {
        Self {
            schema: ToolSchema {
                name: "glob".to_string(),
                description: "Find files matching a glob pattern.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["pattern"],
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern (e.g., \"src/**/*.rs\")"
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory to search in (default: project root)"
                        }
                    }
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
        };

        let base = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| context.project_root.join(p))
            .unwrap_or_else(|| context.project_root.clone());

        let full_pattern = base.join(pattern).to_string_lossy().to_string();

        match glob::glob(&full_pattern) {
            Ok(paths) => {
                let mut matches: Vec<String> = paths
                    .filter_map(|p| p.ok())
                    .filter_map(|p| {
                        p.strip_prefix(&context.project_root)
                            .ok()
                            .map(|rel| rel.to_string_lossy().to_string())
                    })
                    .collect();

                let total = matches.len();
                let truncated = total > 200;
                matches.truncate(200);

                let output = if matches.is_empty() {
                    format!("No files matching pattern: {}", pattern)
                } else {
                    matches.join("\n")
                };

                ToolResult::ok(output)
                    .with_meta("match_count", serde_json::json!(total))
                    .with_meta("truncated", serde_json::json!(truncated))
            }
            Err(e) => ToolResult::err(format!("Invalid glob pattern: {}", e)),
        }
    }
}
