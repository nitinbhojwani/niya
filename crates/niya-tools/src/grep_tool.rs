//! Grep tool — search file contents by regex.

use async_trait::async_trait;
use niya_core::tool::Tool;
use niya_core::types::{ToolContext, ToolResult, ToolSchema};
use walkdir::WalkDir;

pub struct GrepTool {
    schema: ToolSchema,
}

impl GrepTool {
    pub fn new() -> Self {
        Self {
            schema: ToolSchema {
                name: "grep".to_string(),
                description: "Search file contents by regex pattern.".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["pattern"],
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for"
                        },
                        "path": {
                            "type": "string",
                            "description": "File or directory to search (default: project root)"
                        },
                        "glob": {
                            "type": "string",
                            "description": "Filter files by glob pattern"
                        },
                        "max_results": {
                            "type": "integer",
                            "description": "Maximum matches to return (default: 100)"
                        }
                    }
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let pattern = match input.get("pattern").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return ToolResult::err("Missing required parameter: pattern"),
        };

        let search_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| context.project_root.join(p))
            .unwrap_or_else(|| context.project_root.clone());

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => return ToolResult::err(format!("Invalid regex: {}", e)),
        };

        let glob_filter = input.get("glob").and_then(|v| v.as_str());

        let mut matches = Vec::new();
        let mut files_searched = 0;

        for entry in WalkDir::new(&search_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Apply glob filter if specified
            if let Some(glob_pat) = glob_filter {
                let file_name = path.to_string_lossy();
                if let Ok(g) = glob::Pattern::new(glob_pat) {
                    if !g.matches(&file_name) {
                        continue;
                    }
                }
            }

            // Skip binary files (heuristic: check first 512 bytes)
            if let Ok(bytes) = std::fs::read(path) {
                if bytes.len() > 512 && bytes[..512].contains(&0) {
                    continue;
                }
            }

            files_searched += 1;

            if let Ok(content) = std::fs::read_to_string(path) {
                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        let rel_path = path
                            .strip_prefix(&context.project_root)
                            .unwrap_or(path)
                            .to_string_lossy();
                        matches.push(format!("{}:{}: {}", rel_path, line_num + 1, line));

                        if matches.len() >= max_results {
                            break;
                        }
                    }
                }
            }

            if matches.len() >= max_results {
                break;
            }
        }

        let total = matches.len();
        let truncated = total >= max_results;

        let output = if matches.is_empty() {
            format!("No matches for pattern: {}", pattern)
        } else {
            matches.join("\n")
        };

        ToolResult::ok(output)
            .with_meta("match_count", serde_json::json!(total))
            .with_meta("files_searched", serde_json::json!(files_searched))
            .with_meta("truncated", serde_json::json!(truncated))
    }
}
