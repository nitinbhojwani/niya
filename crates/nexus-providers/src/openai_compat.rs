//! OpenAI-compatible provider adapter.
//!
//! Works with OpenAI, Ollama, LM Studio, vLLM, Together AI, and any endpoint
//! that implements the OpenAI Chat Completions API format.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use reqwest::Client;
use std::collections::HashMap;

use nexus_core::provider::ProviderAdapter;
use nexus_core::types::{
    ChatRequest, ChatResponseChunk, Message, AssistantContent, ProviderError,
};

/// Adapter for OpenAI-compatible APIs.
pub struct OpenAiCompatAdapter {
    name: String,
    client: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
    context_window: usize,
}

impl OpenAiCompatAdapter {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
        context_window: usize,
    ) -> Self {
        Self {
            name: name.into(),
            client: Client::new(),
            base_url: base_url.into(),
            api_key,
            model: model.into(),
            context_window,
        }
    }

    /// Build the request body in OpenAI format.
    fn build_request_body(&self, request: &ChatRequest) -> serde_json::Value {
        let mut messages = Vec::new();

        // System message
        messages.push(serde_json::json!({
            "role": "system",
            "content": request.system,
        }));

        // Conversation messages
        for msg in &request.messages {
            match msg {
                Message::User { content } => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                Message::Assistant { content } => {
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();

                    for block in content {
                        match block {
                            AssistantContent::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            AssistantContent::ToolUse { id, name, input } => {
                                tool_calls.push(serde_json::json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                        }
                    }

                    let mut msg = serde_json::json!({
                        "role": "assistant",
                    });
                    if !text_parts.is_empty() {
                        msg["content"] = serde_json::json!(text_parts.join(""));
                    }
                    if !tool_calls.is_empty() {
                        msg["tool_calls"] = serde_json::json!(tool_calls);
                    }
                    messages.push(msg);
                }
                Message::Tool {
                    tool_call_id,
                    content,
                } => {
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": content,
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "model": if request.model.is_empty() { &self.model } else { &request.model },
            "messages": messages,
            "max_tokens": request.max_tokens,
            "stream": true,
        });

        // Add tools if provided
        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools);
        }

        body
    }
}

#[async_trait]
impl ProviderAdapter for OpenAiCompatAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        let url = format!("{}/models", self.base_url);
        let mut req = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) if resp.status().as_u16() == 401 => Err(ProviderError::Auth {
                message: "Invalid API key".to_string(),
            }),
            Ok(resp) => Err(ProviderError::Server {
                message: format!("Unexpected status: {}", resp.status()),
            }),
            Err(e) => Err(ProviderError::Network {
                message: e.to_string(),
            }),
        }
    }

    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'_, ChatResponseChunk>, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = self.build_request_body(&request);

        let mut req = self.client.post(&url).json(&body);
        if let Some(ref key) = self.api_key {
            req = req.bearer_auth(key);
        }

        let response = req.send().await.map_err(|e| ProviderError::Network {
            message: e.to_string(),
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            return Err(if status.as_u16() == 429 {
                ProviderError::RateLimit {
                    message: body,
                    retry_after_ms: None,
                }
            } else if status.as_u16() == 401 {
                ProviderError::Auth { message: body }
            } else {
                ProviderError::Server {
                    message: format!("Status {}: {}", status, body),
                }
            });
        }

        // Parse SSE stream
        let stream = response.bytes_stream();

        let chunk_stream = stream
            .filter_map(|result| async move {
                match result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        parse_sse_chunks(&text)
                    }
                    Err(e) => Some(vec![ChatResponseChunk::Error {
                        error: ProviderError::Network {
                            message: e.to_string(),
                        },
                    }]),
                }
            })
            .flat_map(futures::stream::iter);

        Ok(Box::pin(chunk_stream))
    }

    fn context_window_size(&self) -> usize {
        self.context_window
    }
}

/// Parse Server-Sent Events text into ChatResponseChunk values.
fn parse_sse_chunks(text: &str) -> Option<Vec<ChatResponseChunk>> {
    let mut chunks = Vec::new();
    let mut tool_ids_by_index: HashMap<usize, String> = HashMap::new();
    let mut tool_args_by_index: HashMap<usize, String> = HashMap::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                chunks.push(ChatResponseChunk::Done);
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
                    for choice in choices {
                        if let Some(delta) = choice.get("delta") {
                            // Text content
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                if !content.is_empty() {
                                    chunks.push(ChatResponseChunk::TextDelta {
                                        text: content.to_string(),
                                    });
                                }
                            }

                            // Tool calls
                            if let Some(tool_calls) =
                                delta.get("tool_calls").and_then(|tc| tc.as_array())
                            {
                                for tc in tool_calls {
                                    let index = tc
                                        .get("index")
                                        .and_then(|i| i.as_u64())
                                        .unwrap_or(0) as usize;

                                    if let Some(function) = tc.get("function") {
                                        let id = tc
                                            .get("id")
                                            .and_then(|id| id.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        if !id.is_empty() {
                                            tool_ids_by_index.insert(index, id.clone());
                                        }

                                        let tool_id = tool_ids_by_index
                                            .get(&index)
                                            .cloned()
                                            .unwrap_or_else(|| format!("tool_{}", index));

                                        if let Some(name) =
                                            function.get("name").and_then(|n| n.as_str())
                                        {
                                            chunks.push(ChatResponseChunk::ToolUseStart {
                                                id: tool_id.clone(),
                                                name: name.to_string(),
                                            });
                                        }

                                        if let Some(args) =
                                            function.get("arguments").and_then(|a| a.as_str())
                                        {
                                            if !args.is_empty() {
                                                tool_args_by_index
                                                    .entry(index)
                                                    .and_modify(|existing| {
                                                        existing.push_str(args)
                                                    })
                                                    .or_insert_with(|| args.to_string());

                                                chunks.push(ChatResponseChunk::ToolUseDelta {
                                                    id: tool_id.clone(),
                                                    input_delta: args.to_string(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Check for finish reason
                        if let Some(finish) =
                            choice.get("finish_reason").and_then(|f| f.as_str())
                        {
                            if finish == "tool_calls" {
                                for (index, args) in &tool_args_by_index {
                                    let id = tool_ids_by_index
                                        .get(index)
                                        .cloned()
                                        .unwrap_or_else(|| format!("tool_{}", index));

                                    let parsed_input = serde_json::from_str::<serde_json::Value>(
                                        args,
                                    )
                                    .unwrap_or_else(|_| {
                                        serde_json::json!({ "raw_arguments": args })
                                    });

                                    chunks.push(ChatResponseChunk::ToolUseEnd {
                                        id,
                                        input: parsed_input,
                                    });
                                }
                            }
                        }
                    }
                }

                // Usage info
                if let Some(usage) = parsed.get("usage") {
                    let input = usage
                        .get("prompt_tokens")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0) as u32;
                    let output = usage
                        .get("completion_tokens")
                        .and_then(|t| t.as_u64())
                        .unwrap_or(0) as u32;
                    if input > 0 || output > 0 {
                        chunks.push(ChatResponseChunk::Usage {
                            input_tokens: input,
                            output_tokens: output,
                        });
                    }
                }
            }
        }
    }

    if chunks.is_empty() {
        None
    } else {
        Some(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_text_delta() {
        let sse = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let chunks = parse_sse_chunks(sse).unwrap();
        assert_eq!(chunks.len(), 1);
        assert!(matches!(&chunks[0], ChatResponseChunk::TextDelta { text } if text == "Hello"));
    }

    #[test]
    fn parse_sse_done() {
        let sse = "data: [DONE]";
        let chunks = parse_sse_chunks(sse).unwrap();
        assert!(matches!(chunks[0], ChatResponseChunk::Done));
    }

    #[test]
    fn parse_sse_ignores_empty_lines() {
        let chunks = parse_sse_chunks("\n\n: comment\n");
        assert!(chunks.is_none());
    }

    #[test]
    fn parse_sse_emits_tool_use_end_on_tool_finish() {
        let sse = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"file_read","arguments":"{\"file_path\":\"src/main.rs\"}"}}]},"finish_reason":"tool_calls"}]}"#;

        let chunks = parse_sse_chunks(sse).unwrap();
        assert!(
            chunks
                .iter()
                .any(|c| matches!(c, ChatResponseChunk::ToolUseStart { name, .. } if name == "file_read"))
        );
        assert!(
            chunks
                .iter()
                .any(|c| matches!(c, ChatResponseChunk::ToolUseEnd { id, .. } if id == "call_1"))
        );
    }

    #[test]
    fn build_request_body_includes_system_message() {
        let adapter = OpenAiCompatAdapter::new(
            "test",
            "http://localhost:8080/v1",
            None,
            "test-model",
            128_000,
        );
        let request = ChatRequest {
            system: "You are helpful.".to_string(),
            messages: vec![Message::User {
                content: "Hi".to_string(),
            }],
            tools: vec![],
            model: String::new(),
            max_tokens: 4096,
        };
        let body = adapter.build_request_body(&request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
    }
}
