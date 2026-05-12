//! Shell execute tool.

use async_trait::async_trait;
use niya_core::tool::Tool;
use niya_core::types::{ToolContext, ToolResult, ToolSchema};
use std::process::Stdio;
use tokio::process::Command;

pub struct ShellExecuteTool {
    schema: ToolSchema,
    output_limit: usize,
    default_timeout_ms: u64,
}

impl ShellExecuteTool {
    pub fn new() -> Self {
        Self::with_limits(30_000, 100_000)
    }

    pub fn with_limits(default_timeout_ms: u64, output_limit: usize) -> Self {
        Self {
            schema: ToolSchema {
                name: "shell_execute".to_string(),
                description: "Run a shell command and return stdout, stderr, and exit code."
                    .to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "required": ["command"],
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to run"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory (default: project root)"
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in milliseconds (default: 30000)"
                        }
                    }
                }),
            },
            output_limit,
            default_timeout_ms,
        }
    }
}

#[async_trait]
impl Tool for ShellExecuteTool {
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }

    async fn execute(&self, input: serde_json::Value, context: &ToolContext) -> ToolResult {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::err("Missing required parameter: command"),
        };

        let cwd = input
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|p| context.project_root.join(p))
            .unwrap_or_else(|| context.cwd.clone());

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.default_timeout_ms);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&cwd)
                .envs(&context.env)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                let truncated =
                    stdout.len() + stderr.len() > self.output_limit;
                if truncated {
                    stdout.truncate(self.output_limit / 2);
                    stderr.truncate(self.output_limit / 2);
                }

                let mut out = format!("Exit code: {}\n", exit_code);
                if !stdout.is_empty() {
                    out.push_str(&format!("\n--- stdout ---\n{}\n", stdout));
                }
                if !stderr.is_empty() {
                    out.push_str(&format!("\n--- stderr ---\n{}\n", stderr));
                }

                ToolResult {
                    success: output.status.success(),
                    output: out,
                    metadata: std::collections::HashMap::new(),
                }
                .with_meta("exit_code", serde_json::json!(exit_code))
                .with_meta("truncated", serde_json::json!(truncated))
            }
            Ok(Err(e)) => ToolResult::err(format!("Failed to execute command: {}", e)),
            Err(_) => ToolResult::err(format!(
                "Command timed out after {}ms",
                timeout_ms
            ))
            .with_meta("timed_out", serde_json::json!(true)),
        }
    }
}
