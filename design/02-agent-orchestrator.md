# Niya Component Design: Agent Orchestrator

**Covers Requirements:** TOOL-07, PERM-05 (dry-run), CTX-01 through CTX-03, WKFL-01/02 (Phase 2 prep)  
**Status:** Draft  

---

## 1. Purpose

The Agent Orchestrator is the brain of the system. It receives a user message, assembles context, calls the LLM provider, interprets the response (text and tool calls), dispatches tool executions through the permission gate, feeds results back to the provider, and repeats until the model produces a final response. It contains three sub-components: the Planner (agentic loop logic), the Tool Router (tool dispatch), and the Context Manager (prompt assembly).

---

## 2. Responsibilities

- Run the agentic loop: prompt → response → tool calls → results → prompt → … → final response.
- Enforce a maximum iteration limit to prevent runaway loops.
- Delegate tool dispatch to the Tool Router.
- Delegate permission checks to the Permission Gate.
- Delegate prompt assembly to the Context Manager.
- Support cancellation at any point in the loop.
- Support dry-run mode where tool calls are shown but not executed.

---

## 3. Interfaces

### 3.1 Orchestrator (top-level)

```
interface Orchestrator {
  // Process a user message and yield output events as they occur.
  run(userMessage: string): AsyncIterable<OutputEvent>
  
  // Cancel the current run.
  cancel(): void
  
  // Reset conversation state (for /clear).
  reset(): void

  // Add a file to context mid-session (for /add <path>).
  // Delegates to ContextManager.addFile(). Can be called between turns.
  addFileToContext(filePath: string): Promise<void>
}
```

**Constructor dependencies:**

```
OrchestratorConfig {
  provider:        ProviderAdapter
  toolRegistry:    ToolRegistry
  permissionGate:  PermissionGate
  contextManager:  ContextManager
  sessionLogger:   SessionLogger
  maxIterations:   number          // default: 20
  dryRun:          boolean         // default: false
}
```

### 3.2 Tool Router

The Tool Router is a registry of available tools. The orchestrator looks up tools by name when the provider emits a `tool_use` block.

```
interface ToolRegistry {
  // Register a tool.
  register(tool: Tool): void

  // Look up a tool by name. Returns null if not found.
  get(name: string): Tool | null

  // Return schemas for all registered tools (sent to the provider).
  allSchemas(): ToolSchema[]
}
```

```
interface Tool {
  schema: ToolSchema
  execute(input: Record<string, any>): Promise<ToolResult>
}

interface ToolSchema {
  name:        string
  description: string
  parameters:  JSONSchema    // JSON Schema for the input
}

interface ToolResult {
  success:  boolean
  output:   string           // Human-readable output (shown to model)
  metadata: Record<string, any>  // Structured data (for logging)
}
```

### 3.3 Session Logger

Records every event in the agentic loop for audit and debugging.

```
interface SessionLogger {
  logUserMessage(message: string): void
  logProviderRequest(messages: Message[], model: string): void
  logProviderResponse(response: ProviderResponse): void
  logToolCall(toolName: string, args: Record<string, any>, decision: PermissionDecision): void
  logToolResult(toolName: string, result: ToolResult): void
  logError(error: AgentError): void
  flush(): Promise<void>
}
```

Logs are written as JSON Lines to `<project-root>/.niya/sessions/<session-id>.jsonl`.

---

## 4. Agentic Loop

```
function run(userMessage):
  contextManager.addUserMessage(userMessage)
  sessionLogger.logUserMessage(userMessage)
  iterations = 0

  loop:
    if iterations >= maxIterations:
      yield error("Maximum iterations reached")
      break

    messages = contextManager.assemble()
    toolSchemas = toolRegistry.allSchemas()

    yield status("Thinking...")
    response = provider.chat(messages, toolSchemas)  // streaming

    for each token in response.textStream:
      yield token(token)

    if response.hasToolCalls():
      for each toolCall in response.toolCalls:
        tool = toolRegistry.get(toolCall.name)
        if tool is null:
          result = ToolResult(success=false, output="Unknown tool")
        else:
          yield toolCallEvent(toolCall.name, toolCall.args)
          decision = permissionGate.check(tool.schema, toolCall.args)
          sessionLogger.logToolCall(toolCall.name, toolCall.args, decision)

          if decision == DENIED:
            result = ToolResult(success=false, output="Permission denied by user")
          else if decision == ASK:
            yield approvalRequest(...)  // wait for user
            if approved:
              result = executeTool(tool, toolCall.args)
            else:
              result = ToolResult(success=false, output="User declined")
          else:  // AUTO
            if dryRun:
              result = ToolResult(success=true, output="[dry-run] Would execute")
            else:
              result = executeTool(tool, toolCall.args)

          sessionLogger.logToolResult(toolCall.name, result)
          yield toolResultEvent(toolCall.name, result)

        contextManager.addToolResult(toolCall.id, result)

      iterations++
      continue loop  // go back to provider with tool results

    else:
      // No tool calls — model produced a final response
      contextManager.addAssistantMessage(response.text)
      yield done()
      break
```

---

## 5. Cancellation

When `cancel()` is called:

1. The active provider stream is aborted (HTTP request cancelled).
2. Any in-flight tool execution is **not** interrupted (to avoid leaving the file system in a half-written state). The result is discarded after it completes.
3. The partial assistant response (if any) is added to conversation history so the context remains coherent.
4. Control returns to the REPL prompt.

---

## 6. Dry-Run Mode

When `dryRun` is true, the orchestrator runs the full loop — including provider calls and permission checks — but replaces every `tool.execute()` call with a synthetic result: `"[dry-run] Would execute <toolName> with <args>"`. This lets the user see exactly what the agent would do without side effects.

---

## 7. Error Handling

| Error | Behaviour |
|---|---|
| Provider returns a rate-limit error | Delegate to the Provider Adapter's built-in retry with exponential backoff (see [03-provider-abstraction.md §6](./03-provider-abstraction.md)). Notify the user of the wait. |
| Provider returns an auth or server error | Yield an error event. Do not retry (user must fix the config). |
| Tool execution throws an exception | Capture the error, wrap it in a ToolResult with `success: false`, feed it back to the model so it can adapt. |
| Unknown tool name from provider | Return an error ToolResult. The model will typically self-correct. |
| Max iterations reached | Yield an error event explaining the limit. The user can continue the conversation manually. |

---

## 8. Future Phase Notes

- **Phase 2:** Hooks attach to the `preToolCall` and `postToolCall` lifecycle points inside the loop. Sub-agent spawning adds a `spawnSubAgent()` method that creates a child Orchestrator with its own context. Plan mode wraps the loop in a two-step flow: plan → approve → execute.
- **Phase 3:** The orchestrator becomes deployable as a server endpoint. Session state is serialised/deserialised for persistence.
