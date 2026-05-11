# Nexus Component Design: Provider Abstraction

**Covers Requirements:** PROV-01 through PROV-07  
**Status:** Draft  

---

## 1. Purpose

The Provider Abstraction normalises communication with different LLM backends behind a single interface. The orchestrator sends messages and tool schemas in a canonical format; the provider adapter translates these to the backend's API format, manages the connection, handles streaming, and translates responses back.

---

## 2. Responsibilities

- Translate internal message format ↔ provider-specific API format.
- Manage HTTP/gRPC connections, authentication headers, and base URLs.
- Stream responses token-by-token back to the orchestrator.
- Handle rate-limit responses with exponential backoff and notify the user.
- Validate API keys at startup and surface clear errors on failure.
- Report token usage (input tokens, output tokens) per call.

---

## 3. Interfaces

### 3.1 ProviderAdapter (core interface)

```
interface ProviderAdapter {
  // The provider's identifier (e.g., "anthropic", "openai", "ollama").
  readonly name: string

  // Validate that the provider is reachable and credentials are valid.
  // Throws ProviderAuthError on failure.
  validate(): Promise<void>

  // Send a chat request with tool schemas. Returns a streaming response.
  chat(request: ChatRequest): AsyncIterable<ChatResponseChunk>

  // Return the maximum context window size (in tokens) for the configured model.
  contextWindowSize(): number
}
```

### 3.2 ChatRequest (canonical format)

```
interface ChatRequest {
  messages:  Message[]
  tools:     ToolSchema[]       // available tools for this turn
  model:     string             // model identifier (e.g., "claude-sonnet-4-6")
  maxTokens: number             // max output tokens
  system:    string             // system prompt
}

type Message =
  | { role: "user";      content: string }
  | { role: "assistant"; content: AssistantContent[] }
  | { role: "tool";      toolCallId: string; content: string }

type AssistantContent =
  | { type: "text";     text: string }
  | { type: "tool_use"; id: string; name: string; input: Record<string, any> }
```

### 3.3 ChatResponseChunk (streaming)

```
type ChatResponseChunk =
  | { type: "text_delta";      text: string }
  | { type: "tool_use_start";  id: string; name: string }
  | { type: "tool_use_delta";  id: string; inputDelta: string }
  | { type: "tool_use_end";    id: string; input: Record<string, any> }
  | { type: "usage";           inputTokens: number; outputTokens: number }
  | { type: "done" }
  | { type: "error";           error: ProviderError }
```

### 3.4 ProviderError

```
interface ProviderError {
  code:    "auth" | "rate_limit" | "context_length" | "server" | "network" | "unknown"
  message: string
  retryAfterMs?: number   // present for rate_limit errors
}
```

---

## 4. Adapter Implementations

### 4.1 Anthropic Adapter

Translates to the Anthropic Messages API. Key mappings:

| Internal | Anthropic API |
|---|---|
| `ChatRequest.system` | `system` parameter |
| `Message[role=user]` | `messages[].role = "user"` |
| `ToolSchema` | `tools[]` with `input_schema` |
| `ChatResponseChunk.tool_use_start` | `content_block_start` event with `type: "tool_use"` |

Streaming via SSE (`stream: true`).

### 4.2 OpenAI Adapter

Translates to the OpenAI Chat Completions API. Key mappings:

| Internal | OpenAI API |
|---|---|
| `ChatRequest.system` | `messages[0].role = "system"` |
| `ToolSchema` | `tools[]` with `function.parameters` |
| `tool_use` content | `tool_calls[]` in assistant message |
| Tool results | `messages[].role = "tool"` with `tool_call_id` |

Streaming via SSE (`stream: true`).

### 4.3 Ollama Adapter

Translates to the Ollama `/api/chat` endpoint. Key considerations:

- Tool use support depends on the model. The adapter checks model capabilities at startup and disables tool schemas for models that don't support them.
- For models without native tool use, the adapter injects tool schemas into the system prompt and parses structured output (JSON) from the model's text response.
- Runs against `localhost` by default; base URL is configurable.
- No API key required (unless the user configures one for a remote Ollama instance).

### 4.4 Custom Adapter (OpenAI-compatible)

A generic adapter that works with any OpenAI-compatible API (e.g., LM Studio, vLLM, Together AI). The user provides a base URL and optional API key. It reuses the OpenAI adapter logic with a custom endpoint.

---

## 5. Provider Selection and Configuration

```
// In config.yaml
providers:
  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"      # env var reference
    default_model: "claude-sonnet-4-6"
  openai:
    api_key: "${OPENAI_API_KEY}"
    default_model: "gpt-4o"
  ollama:
    base_url: "http://localhost:11434"
    default_model: "llama3"
  custom:
    base_url: "http://my-server:8080/v1"
    api_key: "${CUSTOM_API_KEY}"
    default_model: "my-model"

default_provider: "anthropic"
```

Selection precedence: `--provider` CLI flag > `--model` flag (infers provider) > project config > global config > default.

---

## 6. Rate Limiting and Retry

```
function chatWithRetry(request, maxRetries=3):
  for attempt in 1..maxRetries:
    try:
      return provider.chat(request)
    catch ProviderError as e:
      if e.code == "rate_limit":
        waitMs = e.retryAfterMs ?? (1000 * 2^attempt)  // exponential backoff
        notify user: "Rate limited. Retrying in {waitMs/1000}s..."
        sleep(waitMs)
      else:
        throw e
  throw MaxRetriesExceeded
```

---

## 7. Token Usage Tracking

Each `ChatResponseChunk` of type `usage` reports token counts. The orchestrator accumulates these across the session and exposes them via the Context Manager for display and logging.

```
interface TokenUsage {
  inputTokens:  number
  outputTokens: number
  estimatedCost: number | null   // null if pricing is unknown for the model
}
```

---

## 8. Startup Validation

At session start, the orchestrator calls `provider.validate()`. This method:

1. Makes a lightweight API call (e.g., list models, or a minimal completion).
2. Verifies the API key is accepted.
3. Verifies the configured model exists.
4. Reports the context window size.

On failure, a clear error message is shown: "Could not connect to Anthropic: invalid API key. Check your ANTHROPIC_API_KEY environment variable."

---

## 9. Future Phase Notes

- **Phase 2:** Model routing — the orchestrator can select different models for different tasks (e.g., a cheaper model for simple edits, a stronger model for complex reasoning). Provider fallback chains (if primary provider fails, try secondary).
- **Phase 3:** Usage tracking feeds into cost dashboards and quota enforcement. Adapters report pricing metadata.
