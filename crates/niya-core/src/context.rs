//! Context manager.
//!
//! Assembles the prompt sent to the LLM provider on every turn. Manages the
//! system prompt, project context, conversation history, pinned files, and
//! token budget tracking.

use std::path::{Path, PathBuf};

use crate::types::{AssistantContent, Message, TokenUsage, ToolResult};

/// Assembled context ready to send to the provider.
#[derive(Debug)]
pub struct AssembledContext {
    /// The system prompt.
    pub system: String,
    /// The conversation history.
    pub messages: Vec<Message>,
    /// Token budget information.
    pub budget: TokenBudget,
}

/// Token budget status.
#[derive(Debug, Clone)]
pub struct TokenBudget {
    pub context_window_size: usize,
    pub used_tokens: usize,
    pub remaining_for_output: usize,
    pub warning_threshold: bool,
}

/// The context manager that builds prompts and tracks conversation history.
pub struct ContextManager {
    system_prompt: String,
    project_instructions: Option<String>,
    project_context: Vec<String>,
    pinned_files: Vec<PinnedFile>,
    conversation: Vec<Message>,
    context_window_size: usize,
    project_instruction_file: String,
    max_project_context_lines: usize,
    usage: TokenUsage,
}

#[derive(Debug, Clone)]
struct PinnedFile {
    path: PathBuf,
    content: String,
}

impl ContextManager {
    pub fn new(context_window_size: usize) -> Self {
        Self {
            system_prompt: default_system_prompt(),
            project_instructions: None,
            project_context: Vec::new(),
            pinned_files: Vec::new(),
            conversation: Vec::new(),
            context_window_size,
            project_instruction_file: "NIYA.md".to_string(),
            max_project_context_lines: 200,
            usage: TokenUsage::default(),
        }
    }

    /// Configure project-context loading behavior.
    pub fn configure_project_context(
        &mut self,
        project_instruction_file: impl Into<String>,
        max_project_context_lines: usize,
    ) {
        self.project_instruction_file = project_instruction_file.into();
        self.max_project_context_lines = max_project_context_lines.max(1);
    }

    /// Initialize with project root — gathers README, NIYA.md, directory tree.
    pub async fn init(&mut self, project_root: &Path) -> anyhow::Result<()> {
        // Load NIYA.md if it exists
        let instructions_path = project_root.join(&self.project_instruction_file);
        if instructions_path.exists() {
            let content = tokio::fs::read_to_string(&instructions_path).await?;
            self.project_instructions = Some(content);
        }

        // Load README if it exists
        for name in &["README.md", "README.rst", "README.txt", "README"] {
            let readme_path = project_root.join(name);
            if readme_path.exists() {
                let content = tokio::fs::read_to_string(&readme_path).await?;
                // Truncate to first 200 lines
                let truncated: String = content
                    .lines()
                    .take(self.max_project_context_lines)
                    .collect::<Vec<_>>()
                    .join("\n");
                self.project_context
                    .push(format!("{}:\n{}", name, truncated));
                break;
            }
        }

        Ok(())
    }

    /// Add a user message to the conversation.
    pub fn add_user_message(&mut self, message: impl Into<String>) {
        self.conversation
            .push(Message::User { content: message.into() });
    }

    /// Add an assistant response to the conversation.
    pub fn add_assistant_message(&mut self, content: Vec<AssistantContent>) {
        self.conversation
            .push(Message::Assistant { content });
    }

    /// Add a tool result to the conversation.
    pub fn add_tool_result(&mut self, tool_call_id: impl Into<String>, result: &ToolResult) {
        self.conversation.push(Message::Tool {
            tool_call_id: tool_call_id.into(),
            content: result.output.clone(),
        });
    }

    /// Add a file to the pinned context.
    pub async fn add_file(&mut self, path: &Path, project_root: &Path) -> anyhow::Result<()> {
        let resolved = project_root.join(path);
        let content = tokio::fs::read_to_string(&resolved).await?;

        // Truncate very large files
        let max_lines = self.max_project_context_lines;
        let truncated: String = content
            .lines()
            .take(max_lines)
            .collect::<Vec<_>>()
            .join("\n");

        self.pinned_files.push(PinnedFile {
            path: resolved,
            content: truncated,
        });

        Ok(())
    }

    /// Assemble the full context for a provider call.
    pub fn assemble(&self) -> AssembledContext {
        let mut system_parts = vec![self.system_prompt.clone()];

        if let Some(ref instructions) = self.project_instructions {
            system_parts.push(format!("\n## Project Instructions\n\n{}", instructions));
        }

        for ctx in &self.project_context {
            system_parts.push(format!("\n## Project Context\n\n{}", ctx));
        }

        for pinned in &self.pinned_files {
            system_parts.push(format!(
                "\n## Pinned File: {}\n\n{}",
                pinned.path.display(),
                pinned.content
            ));
        }

        let system = system_parts.join("\n");
        let used_tokens = estimate_tokens(&system)
            + self.conversation.iter().map(|m| estimate_message_tokens(m)).sum::<usize>();

        let remaining = self.context_window_size.saturating_sub(used_tokens);
        let warning = used_tokens as f64 / self.context_window_size as f64 > 0.80;

        AssembledContext {
            system,
            messages: self.conversation.clone(),
            budget: TokenBudget {
                context_window_size: self.context_window_size,
                used_tokens,
                remaining_for_output: remaining,
                warning_threshold: warning,
            },
        }
    }

    /// Get current token usage stats.
    pub fn usage(&self) -> &TokenUsage {
        &self.usage
    }

    /// Record token usage from a provider response.
    pub fn record_usage(&mut self, input: u32, output: u32) {
        self.usage.add(input, output);
    }

    /// Clear conversation history.
    pub fn reset(&mut self) {
        self.conversation.clear();
        self.pinned_files.clear();
        self.usage = TokenUsage::default();
    }

    /// Number of turns in the conversation.
    pub fn turn_count(&self) -> usize {
        self.conversation
            .iter()
            .filter(|m| matches!(m, Message::User { .. }))
            .count()
    }
}

/// Rough token estimation: 1 token ≈ 3.5 characters (conservative).
fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 3.5).ceil() as usize
}

fn estimate_message_tokens(message: &Message) -> usize {
    match message {
        Message::User { content } => estimate_tokens(content),
        Message::Assistant { content } => content
            .iter()
            .map(|c| match c {
                AssistantContent::Text { text } => estimate_tokens(text),
                AssistantContent::ToolUse { input, .. } => {
                    estimate_tokens(&input.to_string())
                }
            })
            .sum(),
        Message::Tool { content, .. } => estimate_tokens(content),
    }
}

fn default_system_prompt() -> String {
    r#"You are Niya, a coding assistant that helps developers by reading, writing, and editing files, running shell commands, and searching codebases. You have access to tools for interacting with the project.

Guidelines:
- Always read files before editing them.
- Use targeted edits (file_edit) instead of rewriting entire files when possible.
- Run tests after making changes to verify correctness.
- Explain what you're doing before taking action.
- If a tool call fails, try to understand why and adapt."#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_manager_starts_empty() {
        let cm = ContextManager::new(200_000);
        assert_eq!(cm.turn_count(), 0);
        assert_eq!(cm.usage().total(), 0);
    }

    #[test]
    fn add_user_message_increments_turns() {
        let mut cm = ContextManager::new(200_000);
        cm.add_user_message("Hello");
        cm.add_user_message("Fix the bug");
        assert_eq!(cm.turn_count(), 2);
    }

    #[test]
    fn assemble_includes_system_prompt() {
        let cm = ContextManager::new(200_000);
        let assembled = cm.assemble();
        assert!(assembled.system.contains("Niya"));
    }

    #[test]
    fn reset_clears_everything() {
        let mut cm = ContextManager::new(200_000);
        cm.add_user_message("Hello");
        cm.record_usage(100, 50);
        cm.reset();
        assert_eq!(cm.turn_count(), 0);
        assert_eq!(cm.usage().total(), 0);
    }

    #[test]
    fn token_budget_warns_at_80_percent() {
        let mut cm = ContextManager::new(100); // tiny window for testing
        // Add a large message to push past 80%
        cm.add_user_message("x".repeat(300)); // ~85 tokens at 3.5 chars/token
        let assembled = cm.assemble();
        assert!(assembled.budget.warning_threshold);
    }
}
