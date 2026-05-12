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
                    error: NiyaError::MaxIterations(self.config.max_iterations),
                });
                break;
            }

            if self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
                events.push(OutputEvent::Error {
                    error: NiyaError::Cancelled,
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
                        error: NiyaError::Provider(e),
                    });
                    break;
                }
            };

            // Process the stream
            let mut text_buffer = String::new();
            let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();
            let mut current_tool_name = String::new();
            let mut current_tool_input = String::new();

            tokio::pin!(stream);
            while let Some(chunk) = stream.next().await {
                match chunk {
                    ChatResponseChunk::TextDelta { text } => {
                        text_buffer.push_str(&text);
                        events.push(OutputEvent::Token { text });
                    }
                    ChatResponseChunk::ToolUseStart { id: _, name } => {
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
                            error: NiyaError::Provider(error),
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

                                if let Some(tx) = &approval_tx {
                                    let _ = tx.send(true).await;
                                }

                                let approved = if let Some(rx) = approval_rx.as_mut() {
                                    rx.recv().await.unwrap_or(false)
                                } else {
                                    false
                                };

                                if approved {
                                    if self.config.dry_run {
                                        ToolResult::ok(format!(
                                            "[dry-run] Would execute {} with {}",
                                            name, input
                                        ))
                                    } else {
                                        tool.execute(input.clone(), &self.tool_context).await
                                    }
                                } else {
                                    ToolResult::err("Permission denied by user")
                                }
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
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use niya_config::{PermissionPolicy, ToolPermission};
    use std::collections::{HashMap, VecDeque};
    use tempfile::TempDir;

    struct EchoTool {
        schema: ToolSchema,
    }

    impl EchoTool {
        fn new() -> Self {
            Self {
                schema: ToolSchema {
                    name: "echo_tool".to_string(),
                    description: "Echo input".to_string(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            }
        }
    }

    #[async_trait]
    impl crate::tool::Tool for EchoTool {
        fn schema(&self) -> &ToolSchema {
            &self.schema
        }

        async fn execute(
            &self,
            input: serde_json::Value,
            _context: &ToolContext,
        ) -> ToolResult {
            ToolResult::ok(format!("echo:{}", input))
        }
    }

    struct TestProvider {
        responses: std::sync::Mutex<VecDeque<Vec<ChatResponseChunk>>>,
    }

    impl TestProvider {
        fn new(responses: Vec<Vec<ChatResponseChunk>>) -> Self {
            Self {
                responses: std::sync::Mutex::new(VecDeque::from(responses)),
            }
        }
    }

    #[async_trait]
    impl ProviderAdapter for TestProvider {
        fn name(&self) -> &str {
            "test"
        }

        async fn validate(&self) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn chat(
            &self,
            _request: ChatRequest,
        ) -> Result<BoxStream<'_, ChatResponseChunk>, ProviderError> {
            let mut guard = self.responses.lock().unwrap();
            let chunks = guard.pop_front().unwrap_or_else(|| vec![ChatResponseChunk::Done]);
            Ok(Box::pin(futures::stream::iter(chunks)))
        }

        fn context_window_size(&self) -> usize {
            128_000
        }
    }

    fn allow_policy_for(tool_name: &str) -> PermissionPolicy {
        let mut tools = HashMap::new();
        tools.insert(
            tool_name.to_string(),
            ToolPermission {
                level: "auto".to_string(),
                auto_approve_when: vec![],
            },
        );

        PermissionPolicy {
            default_level: "deny".to_string(),
            tools,
            shell_deny_patterns: vec![],
            allowed_paths: vec![],
        }
    }

    // Integration tests for the orchestrator will go here once
    // we have the mock provider wired up end-to-end.

    #[test]
    fn default_config_has_sensible_values() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_iterations, 20);
        assert!(!config.dry_run);
    }

    #[tokio::test]
    async fn executes_tool_call_when_tool_use_end_is_present() {
        let temp_dir = TempDir::new().unwrap();

        let provider = TestProvider::new(vec![
            vec![
                ChatResponseChunk::ToolUseStart {
                    id: "call_1".to_string(),
                    name: "echo_tool".to_string(),
                },
                ChatResponseChunk::ToolUseDelta {
                    id: "call_1".to_string(),
                    input_delta: "{\"msg\":\"hello\"}".to_string(),
                },
                ChatResponseChunk::ToolUseEnd {
                    id: "call_1".to_string(),
                    input: serde_json::json!({"msg": "hello"}),
                },
                ChatResponseChunk::Done,
            ],
            vec![
                ChatResponseChunk::TextDelta {
                    text: "done".to_string(),
                },
                ChatResponseChunk::Done,
            ],
        ]);

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool::new()));

        let permission_gate = PermissionGate::new(
            allow_policy_for("echo_tool"),
            temp_dir.path().to_path_buf(),
        );

        let context_manager = ContextManager::new(128_000);
        let session_logger = SessionLogger::new("test-session", None);
        let tool_context = ToolContext {
            project_root: temp_dir.path().to_path_buf(),
            cwd: temp_dir.path().to_path_buf(),
            env: HashMap::new(),
        };

        let mut orchestrator = Orchestrator::new(
            Box::new(provider),
            registry,
            permission_gate,
            context_manager,
            session_logger,
            OrchestratorConfig::default(),
            tool_context,
        );

        let mut approval_rx = None;
        let events = orchestrator
            .run("use tool".to_string(), None, &mut approval_rx)
            .await;

        assert!(events.iter().any(|e| {
            matches!(
                e,
                OutputEvent::ToolCall { tool_name, .. } if tool_name == "echo_tool"
            )
        }));
        assert!(events.iter().any(|e| {
            matches!(
                e,
                OutputEvent::ToolResult { result, .. } if result.success
            )
        }));
        assert!(events.iter().any(|e| matches!(e, OutputEvent::Done)));
    }
}
