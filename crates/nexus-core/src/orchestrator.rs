//! Agent orchestrator — the agentic loop.
//!
//! Receives user messages, calls the LLM provider, dispatches tool calls
//! through the permission gate, feeds results back, and repeats until
//! the model produces a final text response.

use futures::StreamExt;
use tokio::sync::mpsc;

use crate::context::ContextManager;
use crate::permission::PermissionGate;
use crate::provider::ProviderAdapter;
use crate::session::SessionLogger;
use crate::tool::ToolRegistry;
use crate::types::*;

/// Configuration for the orchestrator.
pub struct OrchestratorConfig {
    pub max_iterations: usize,
    pub dry_run: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            dry_run: false,
        }
    }
}

/// The agent orchestrator that runs the agentic loop.
pub struct Orchestrator {
    provider: Box<dyn ProviderAdapter>,
    tool_registry: ToolRegistry,
    permission_gate: PermissionGate,
    context_manager: ContextManager,
    session_logger: SessionLogger,
    config: OrchestratorConfig,
    tool_context: ToolContext,
    cancelled: std::sync::atomic::AtomicBool,
}

impl Orchestrator {
    pub fn new(
        provider: Box<dyn ProviderAdapter>,
        tool_registry: ToolRegistry,
        permission_gate: PermissionGate,
        context_manager: ContextManager,
        session_logger: SessionLogger,
        config: OrchestratorConfig,
        tool_context: ToolContext,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            permission_gate,
            context_manager,
            session_logger,
            config,
            tool_context,
            cancelled: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Run one user turn through the agentic loop.
    ///
    /// Sends output events through the returned receiver as they occur.
    pub async fn run(
        &mut self,
        user_message: String,
        approval_tx: Option<mpsc::Sender<bool>>,
        approval_rx: &mut Option<mpsc::Receiver<bool>>,
    ) -> Vec<OutputEvent> {
        self.cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);

        self.context_manager.add_user_message(&user_message);
        self.session_logger.log_user_message(&user_message);

        let mut events = Vec::new();
        let mut iterations = 0;

        loop {
            if iterations >= self.config.max_iterations {
                events.push(OutputEvent::Error {
                    error: NexusError::MaxIterations(self.config.max_iterations),
                });
                break;
            }

            if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                events.push(OutputEvent::Error {
                    error: NexusError::Cancelled,
                });
                break;
            }

            // Assemble context and call provider
            let assembled = self.context_manager.assemble();
            let request = ChatRequest {
                system: assembled.system,
                messages: assembled.messages,
                tools: self.tool_registry.all_schemas(),
                model: String::new(), // filled by provider
                max_tokens: assembled.budget.remaining_for_output.min(8192) as u32,
            };

            events.push(OutputEvent::Status {
                message: "Thinking...".to_string(),
            });

            let stream = match self.provider.chat(request).await {
                Ok(s) => s,
                Err(e) => {
                    events.push(OutputEvent::Error {
                        error: NexusError::Provider(e),
                    });
                    break;
                }
            };

            // Process the stream
            let mut text_buffer = String::new();
            let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
            let mut current_tool_id = String::new();
            let mut current_tool_name = String::new();
            let mut current_tool_input = String::new();

            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                match chunk {
                    ChatResponseChunk::TextDelta { text } => {
                        text_buffer.push_str(&text);
                        events.push(OutputEvent::Token { text });
                    }
                    ChatResponseChunk::ToolUseStart { id, name } => {
                        current_tool_id = id;
                        current_tool_name = name;
                        current_tool_input.clear();
                    }
                    ChatResponseChunk::ToolUseDelta { input_delta, .. } => {
                        current_tool_input.push_str(&input_delta);
                    }
                    ChatResponseChunk::ToolUseEnd { id, input } => {
                        tool_calls.push((id, current_tool_name.clone(), input));
                    }
                    ChatResponseChunk::Usage {
                        input_tokens,
                        output_tokens,
                    } => {
                        self.context_manager
                            .record_usage(input_tokens, output_tokens);
                    }
                    ChatResponseChunk::Done => break,
                    ChatResponseChunk::Error { error } => {
                        events.push(OutputEvent::Error {
                            error: NexusError::Provider(error),
                        });
                        break;
                    }
                }
            }

            // If there are tool calls, execute them
            if !tool_calls.is_empty() {
                // Build the assistant message with text + tool_use blocks
                let mut assistant_content = Vec::new();
                if !text_buffer.is_empty() {
                    assistant_content.push(AssistantContent::Text {
                        text: text_buffer.clone(),
                    });
                }
                for (id, name, input) in &tool_calls {
                    assistant_content.push(AssistantContent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
                self.context_manager
                    .add_assistant_message(assistant_content);

                // Execute each tool call
                for (id, name, input) in &tool_calls {
                    let tool = self.tool_registry.get(name);
                    let result = if let Some(tool) = tool {
                        events.push(OutputEvent::ToolCall {
                            tool_name: name.clone(),
                            args: input.clone(),
                        });

                        let decision =
                            self.permission_gate.check(tool.schema(), input);
                        self.session_logger
                            .log_tool_call(name, input, &decision);

                        match decision {
                            PermissionDecision::Allow => {
                                if self.config.dry_run {
                                    ToolResult::ok(format!(
                                        "[dry-run] Would execute {} with {}",
                                        name, input
                                    ))
                                } else {
                                    tool.execute(input.clone(), &self.tool_context).await
                                }
                            }
                            PermissionDecision::Deny { reason } => {
                                ToolResult::err(format!("Permission denied: {}", reason))
                            }
                            PermissionDecision::Ask { message } => {
                                events.push(OutputEvent::ApprovalRequest {
                                    tool_name: name.clone(),
                                    args: input.clone(),
                                    message,
                                });

                                // In a real implementation, we'd wait for user input here.
                                // For now, default to deny if no approval channel.
                                // TODO: wire up approval channel from CLI
                                ToolResult::err("Awaiting user approval (not yet wired)")
                            }
                        }
                    } else {
                        ToolResult::err(format!("Unknown tool: {}", name))
                    };

                    self.session_logger.log_tool_result(name, &result);
                    events.push(OutputEvent::ToolResult {
                        tool_name: name.clone(),
                        result: result.clone(),
                    });
                    self.context_manager.add_tool_result(id, &result);
                }

                iterations += 1;
                continue; // Loop back to provider with tool results
            } else {
                // No tool calls — final response
                if !text_buffer.is_empty() {
                    self.context_manager.add_assistant_message(vec![
                        AssistantContent::Text { text: text_buffer },
                    ]);
                }
                events.push(OutputEvent::Done);
                break;
            }
        }

        events
    }

    /// Cancel the current run.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Reset conversation state.
    pub fn reset(&mut self) {
        self.context_manager.reset();
    }

    /// Add a file to context mid-session.
    pub async fn add_file_to_context(
        &mut self,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        self.context_manager
            .add_file(path, &self.tool_context.project_root)
            .await
    }

    /// Get accumulated token usage.
    pub fn usage(&self) -> &TokenUsage {
        self.context_manager.usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests for the orchestrator will go here once
    // we have the mock provider wired up end-to-end.

    #[test]
    fn default_config_has_sensible_values() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!(!config.dry_run);
    }
}
