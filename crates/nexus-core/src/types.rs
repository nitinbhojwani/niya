//! Core types used across all Nexus components.
//!
//! This module defines the canonical data structures for messages, tool calls,
//! provider communication, and output events. All components depend on these
//! types — they form the shared language of the system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Messages (the conversation format)
// ---------------------------------------------------------------------------

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User { content: String },

    #[serde(rename = "assistant")]
    Assistant { content: Vec<AssistantContent> },

    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
    },
}

/// Content blocks within an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

// ---------------------------------------------------------------------------
// Tool schemas and results
// ---------------------------------------------------------------------------

/// Schema describing a tool's interface, sent to the LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

/// Result returned by a tool after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create an error result.
    pub fn err(output: impl Into<String>) -> Self {
        Self {
            success: false,
            output: output.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add a metadata entry.
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

// ---------------------------------------------------------------------------
// Provider communication
// ---------------------------------------------------------------------------

/// A request to the LLM provider.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
    pub model: String,
    pub max_tokens: u32,
}

/// A streaming chunk from the provider response.
#[derive(Debug, Clone)]
pub enum ChatResponseChunk {
    /// A piece of text output.
    TextDelta { text: String },

    /// A tool call is starting.
    ToolUseStart { id: String, name: String },

    /// Incremental JSON input for a tool call.
    ToolUseDelta { id: String, input_delta: String },

    /// A tool call is complete with parsed input.
    ToolUseEnd { id: String, input: serde_json::Value },

    /// Token usage for this response.
    Usage {
        input_tokens: u32,
        output_tokens: u32,
    },

    /// The response is complete.
    Done,

    /// An error occurred.
    Error { error: ProviderError },
}

/// An error from the LLM provider.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed: {message}")]
    Auth { message: String },

    #[error("Rate limited: {message}")]
    RateLimit {
        message: String,
        retry_after_ms: Option<u64>,
    },

    #[error("Context length exceeded: {message}")]
    ContextLength { message: String },

    #[error("Server error: {message}")]
    Server { message: String },

    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

// ---------------------------------------------------------------------------
// Permission decisions
// ---------------------------------------------------------------------------

/// The result of a permission check for a tool invocation.
#[derive(Debug, Clone)]
pub enum PermissionDecision {
    /// The tool call is allowed.
    Allow,

    /// The tool call is denied.
    Deny { reason: String },

    /// The user must be asked for approval.
    Ask { message: String },
}

// ---------------------------------------------------------------------------
// Output events (orchestrator → CLI)
// ---------------------------------------------------------------------------

/// Events emitted by the orchestrator, consumed by the CLI for rendering.
#[derive(Debug)]
pub enum OutputEvent {
    /// A token of text from the model.
    Token { text: String },

    /// A tool is about to be called.
    ToolCall {
        tool_name: String,
        args: serde_json::Value,
    },

    /// A tool has returned a result.
    ToolResult {
        tool_name: String,
        result: crate::types::ToolResult,
    },

    /// The user must approve a tool call.
    ApprovalRequest {
        tool_name: String,
        args: serde_json::Value,
        message: String,
    },

    /// A status message (e.g., "Thinking...").
    Status { message: String },

    /// An error occurred.
    Error { error: NexusError },

    /// The current turn is complete.
    Done,
}

// ---------------------------------------------------------------------------
// Context for tool execution
// ---------------------------------------------------------------------------

/// Runtime context passed to tools during execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Absolute path to the project root directory.
    pub project_root: std::path::PathBuf,

    /// Current working directory (usually same as project_root).
    pub cwd: std::path::PathBuf,

    /// Environment variables available to shell commands.
    pub env: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Token usage tracking
// ---------------------------------------------------------------------------

/// Accumulated token usage for a session.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    pub fn add(&mut self, input: u32, output: u32) {
        self.input_tokens += input as u64;
        self.output_tokens += output as u64;
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Top-level error type for Nexus.
#[derive(Debug, thiserror::Error)]
pub enum NexusError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Permission denied: {0}")]
    Permission(String),

    #[error("Context overflow: used {used} tokens, limit is {limit}")]
    ContextOverflow { used: usize, limit: usize },

    #[error("Max iterations reached: {0}")]
    MaxIterations(usize),

    #[error("Cancelled by user")]
    Cancelled,

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_ok_creates_success() {
        let result = ToolResult::ok("file read successfully");
        assert!(result.success);
        assert_eq!(result.output, "file read successfully");
    }

    #[test]
    fn tool_result_err_creates_failure() {
        let result = ToolResult::err("file not found");
        assert!(!result.success);
    }

    #[test]
    fn tool_result_with_meta_adds_metadata() {
        let result = ToolResult::ok("done")
            .with_meta("path", serde_json::json!("/src/main.rs"))
            .with_meta("lines", serde_json::json!(42));
        assert_eq!(result.metadata.len(), 2);
    }

    #[test]
    fn token_usage_tracks_totals() {
        let mut usage = TokenUsage::default();
        usage.add(100, 50);
        usage.add(200, 75);
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 125);
        assert_eq!(usage.total(), 425);
    }
}
