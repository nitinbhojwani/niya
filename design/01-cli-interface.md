# Nexus Component Design: CLI Interface

**Covers Requirements:** CLI-01 through CLI-07  
**Status:** Draft  

---

## 1. Purpose

The CLI Interface is the user-facing layer. It handles argument parsing, runs the interactive REPL, streams output to the terminal, renders markdown formatting, and manages user prompts for permission approvals. It is a thin shell over the Agent Orchestrator — it contains no business logic.

---

## 2. Responsibilities

- Parse command-line arguments and flags.
- Launch the Agent Orchestrator in either one-shot mode (prompt as argument) or interactive REPL mode.
- Read from stdin when piped input is detected.
- Stream provider responses token-by-token to the terminal.
- Render markdown (headings, bold, code blocks, inline code) in the terminal.
- Display tool invocations distinctly from model output (spinners, labels, collapsible diffs).
- Handle OS signals: Ctrl-C (cancel current generation), Ctrl-D (exit session).
- Prompt the user for yes/no approval when the Permission Gate requires it.

---

## 3. Interfaces

### 3.1 CLI Entry Point

```
nexus [options] [prompt]

Options:
  --provider <name>       Override the default LLM provider
  --model <name>          Override the default model
  --config <path>         Path to a config file
  --project-root <path>   Override auto-detected project root
  --dry-run               Preview actions without executing
  --verbose               Show debug-level output
  --no-color              Disable terminal colors
  --version               Print version and exit
  --help                  Print help and exit
```

### 3.2 InputSource (interface)

Abstracts where user input comes from, enabling the REPL, piped stdin, and one-shot modes to share the same orchestrator interface.

```
interface InputSource {
  // Returns the next user message. Resolves to null on EOF / exit.
  nextMessage(): Promise<string | null>

  // Whether the source is interactive (REPL) vs. non-interactive (pipe, one-shot).
  isInteractive(): boolean
}
```

**Implementations:**
- `ReplInputSource` — reads from a readline interface with history, line editing, and multi-line support.
- `PipeInputSource` — reads all of stdin, yields it as one message, then returns null.
- `OneShotInputSource` — yields the CLI positional argument as one message, then returns null.

### 3.3 OutputRenderer (interface)

Abstracts terminal output so the orchestrator doesn't depend on terminal details.

```
interface OutputRenderer {
  // Stream a token of model text output.
  streamToken(token: string): void

  // Signal that model output is complete for this turn.
  endStream(): void

  // Display a tool invocation (before execution).
  showToolCall(toolName: string, args: Record<string, any>): void

  // Display a tool result (after execution).
  showToolResult(toolName: string, result: ToolResult): void

  // Display an error message.
  showError(error: AgentError): void

  // Prompt the user for a yes/no decision. Returns the answer.
  promptApproval(message: string): Promise<boolean>

  // Display a status/progress indicator (e.g., spinner).
  showStatus(message: string): void

  // Clear the current status indicator.
  clearStatus(): void
}
```

### 3.4 Events Emitted to Orchestrator

The CLI Interface does not call tools or providers directly. It communicates with the Orchestrator through a simple request/callback model:

```
interface SessionCallbacks {
  // Called when the user submits a message.
  onUserMessage(message: string): AsyncIterable<OutputEvent>
  
  // Called when the user requests cancellation (Ctrl-C).
  onCancel(): void
  
  // Called when the user exits (Ctrl-D or /exit).
  onExit(): void
}
```

`OutputEvent` is a discriminated union:

```
type OutputEvent =
  | { type: "token";      text: string }
  | { type: "tool_call";  toolName: string; args: Record<string, any> }
  | { type: "tool_result"; toolName: string; result: ToolResult }
  | { type: "approval_request"; message: string; resolve: (approved: boolean) => void }
  | { type: "error";      error: AgentError }
  | { type: "done" }
```

---

## 4. Markdown Rendering

Terminal markdown rendering covers a practical subset:

| Markdown Element | Terminal Rendering |
|---|---|
| `# Heading` | Bold + newline padding |
| `**bold**` | Bold (ANSI) |
| `` `inline code` `` | Highlighted background |
| ```` ```code block``` ```` | Indented, syntax-highlighted (if library available), bordered |
| `- list item` | Preserved as-is |
| `> blockquote` | Dimmed or indented |

A `--no-color` flag disables all ANSI formatting and outputs plain text.

---

## 5. Session Lifecycle

```
1. Parse CLI arguments
2. Load configuration (delegate to Configuration System)
3. Determine InputSource (REPL / pipe / one-shot)
4. Create OutputRenderer (terminal / plain-text based on flags)
5. Initialize Agent Orchestrator with config, InputSource callbacks
6. Loop:
   a. Read next message from InputSource
   b. If null → exit
   c. If message starts with "/" → handle as slash command (Phase 2 extension point)
   d. Pass message to Orchestrator.onUserMessage()
   e. Iterate OutputEvents, dispatch to OutputRenderer
7. On exit: flush session log, print summary (tokens used, files changed), clean up
```

---

## 6. Slash Commands (Extension Point)

In the MVP, only a few built-in commands are supported:

| Command | Action |
|---|---|
| `/exit` or `/quit` | End the session |
| `/clear` | Clear conversation history and start fresh |
| `/add <path>` | Add a file to context (delegates to Context Manager) |
| `/help` | Show available commands |
| `/config` | Show current configuration |

The command dispatch table is a simple map. Phase 2 will extend this with user-defined commands.

---

## 7. Error Handling

| Error Case | Behaviour |
|---|---|
| Provider connection failure | Show error, suggest checking API key / network. Do not crash. |
| User cancels mid-generation (Ctrl-C) | Abort the current provider stream. Return to the prompt. Conversation history includes the partial response. |
| Invalid CLI arguments | Print usage help and exit with code 1. |
| Permission denied for a tool | Show denial to user, return error to model so it can adapt. |

---

## 8. Future Phase Notes

- **Phase 2:** Slash commands become extensible. Hooks fire at session-start/end. The OutputRenderer interface enables an IDE integration to swap in a different renderer.
- **Phase 3:** The CLI can connect to a remote orchestrator (centralised deployment) instead of running locally.
