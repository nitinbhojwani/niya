//! Session logger.
//!
//! Records every event in the agentic loop for audit and debugging.
//! Writes structured JSON Lines to a session log file.

use chrono::Utc;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::types::{PermissionDecision, ToolResult};

/// Logger that records session events to a JSON Lines file.
pub struct SessionLogger {
    session_id: String,
    log_path: Option<PathBuf>,
    entries: Vec<serde_json::Value>,
}

impl SessionLogger {
    /// Create a new session logger.
    ///
    /// If `log_dir` is provided, a log file will be created at
    /// `<log_dir>/<session_id>.jsonl`. Otherwise, events are kept in memory only.
    pub fn new(session_id: impl Into<String>, log_dir: Option<&Path>) -> Self {
        let session_id = session_id.into();
        let log_path = log_dir.map(|dir| dir.join(format!("{}.jsonl", session_id)));

        Self {
            session_id,
            log_path,
            entries: Vec::new(),
        }
    }

    /// Log a user message.
    pub fn log_user_message(&mut self, message: &str) {
        self.append(json!({
            "event": "user_message",
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "message": message,
        }));
    }

    /// Log a tool invocation with the permission decision.
    pub fn log_tool_call(
        &mut self,
        tool_name: &str,
        args: &serde_json::Value,
        decision: &PermissionDecision,
    ) {
        let decision_str = match decision {
            PermissionDecision::Allow => "allow",
            PermissionDecision::Deny { .. } => "deny",
            PermissionDecision::Ask { .. } => "ask",
        };

        self.append(json!({
            "event": "tool_call",
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "tool": tool_name,
            "args": args,
            "decision": decision_str,
        }));
    }

    /// Log a tool result.
    pub fn log_tool_result(&mut self, tool_name: &str, result: &ToolResult) {
        self.append(json!({
            "event": "tool_result",
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "tool": tool_name,
            "success": result.success,
            "output_length": result.output.len(),
        }));
    }

    /// Log an error.
    pub fn log_error(&mut self, error: &str) {
        self.append(json!({
            "event": "error",
            "timestamp": Utc::now().to_rfc3339(),
            "session_id": self.session_id,
            "error": error,
        }));
    }

    /// Flush all buffered entries to disk.
    pub fn flush(&self) -> anyhow::Result<()> {
        if let Some(ref path) = self.log_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;

            for entry in &self.entries {
                writeln!(file, "{}", entry)?;
            }
        }
        Ok(())
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get all logged entries (for testing).
    pub fn entries(&self) -> &[serde_json::Value] {
        &self.entries
    }

    fn append(&mut self, entry: serde_json::Value) {
        self.entries.push(entry);
    }
}

impl Drop for SessionLogger {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_records_user_messages() {
        let mut logger = SessionLogger::new("test-session", None);
        logger.log_user_message("Hello, world!");
        assert_eq!(logger.entries().len(), 1);
        assert_eq!(logger.entries()[0]["event"], "user_message");
        assert_eq!(logger.entries()[0]["message"], "Hello, world!");
    }

    #[test]
    fn logger_records_tool_calls() {
        let mut logger = SessionLogger::new("test-session", None);
        logger.log_tool_call(
            "file_read",
            &serde_json::json!({"file_path": "src/main.rs"}),
            &PermissionDecision::Allow,
        );
        assert_eq!(logger.entries()[0]["event"], "tool_call");
        assert_eq!(logger.entries()[0]["decision"], "allow");
    }

    #[test]
    fn logger_records_tool_results() {
        let mut logger = SessionLogger::new("test-session", None);
        let result = ToolResult::ok("file contents here");
        logger.log_tool_result("file_read", &result);
        assert_eq!(logger.entries()[0]["event"], "tool_result");
        assert_eq!(logger.entries()[0]["success"], true);
    }
}
