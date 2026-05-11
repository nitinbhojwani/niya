# Nexus Component Design: Context Manager

**Covers Requirements:** CTX-01 through CTX-05  
**Status:** Draft  

---

## 1. Purpose

The Context Manager assembles the prompt sent to the LLM provider on every turn. It is responsible for combining the system prompt, project-level instructions, automatically gathered project context, conversation history, and tool results into a single message array that fits within the provider's context window.

---

## 2. Responsibilities

- Build and maintain the system prompt (static instructions + tool usage guidelines).
- Auto-detect and include project context (README, directory structure, NEXUS.md).
- Maintain the conversation history (user messages, assistant messages, tool results).
- Track cumulative token usage and warn when approaching the context window limit.
- Allow the user to manually add files to context (`/add`).
- Assemble the final `Message[]` array for each provider call.

---

## 3. Interfaces

### 3.1 ContextManager

```
interface ContextManager {
  // Initialize with project root and provider context window size.
  init(projectRoot: string, contextWindowSize: number): Promise<void>

  // Add a user message to conversation history.
  addUserMessage(message: string): void

  // Add an assistant response to conversation history.
  addAssistantMessage(content: AssistantContent[]): void

  // Add a tool result to conversation history.
  addToolResult(toolCallId: string, result: ToolResult): void

  // Manually add a file's contents to the pinned context.
  addFile(filePath: string): Promise<void>

  // Assemble the full message array for the next provider call.
  assemble(): AssembledContext

  // Get current token usage stats.
  getUsage(): TokenUsageStats

  // Reset conversation history (for /clear).
  reset(): void
}

interface AssembledContext {
  system:   string       // system prompt
  messages: Message[]    // conversation history
  budget:   TokenBudget  // how many tokens remain for the response
}

interface TokenBudget {
  contextWindowSize:  number
  usedTokens:         number    // estimated tokens in the assembled prompt
  remainingForOutput: number    // tokens available for the model's response
  warningThreshold:   boolean   // true if >80% of context is consumed
}

interface TokenUsageStats {
  totalInputTokens:  number
  totalOutputTokens: number
  totalTokens:       number
  estimatedCost:     number | null
  turns:             number
}
```

---

## 4. Context Assembly

The assembled prompt has a defined structure:

```
┌─────────────────────────────────────────────┐
│ 1. System Prompt (static)                   │
│    - Agent identity and behaviour rules     │
│    - Tool usage guidelines                  │
│    - Output format instructions             │
├─────────────────────────────────────────────┤
│ 2. Project Instructions (if NEXUS.md exists)│
│    - Project-specific conventions           │
│    - Preferred libraries, patterns          │
├─────────────────────────────────────────────┤
│ 3. Project Context (auto-gathered)          │
│    - Directory tree (top 3 levels)          │
│    - README.md (first 200 lines)           │
│    - Key config files (package.json, etc.)  │
├─────────────────────────────────────────────┤
│ 4. Pinned Files (user-added via /add)       │
│    - Full contents of manually added files  │
├─────────────────────────────────────────────┤
│ 5. Conversation History                     │
│    - user / assistant / tool_result turns   │
└─────────────────────────────────────────────┘
```

### Priority under token pressure

When the assembled context approaches the window limit, sections are trimmed in this order (lowest priority first):

1. **Project context** — directory tree depth is reduced, README is truncated further.
2. **Pinned files** — oldest pinned files are dropped first, with a note to the model.
3. **Early conversation history** — oldest turns are summarised into a single "conversation so far" block.
4. **System prompt** — never trimmed.

---

## 5. Auto-Gathered Project Context

On `init()`, the Context Manager scans the project root for useful context:

```
function gatherProjectContext(projectRoot):
  context = []

  // Directory tree (top 3 levels, respecting .gitignore)
  tree = generateDirectoryTree(projectRoot, maxDepth=3, respectGitignore=true)
  context.add("Directory structure:\n" + tree)

  // README
  readme = findFile(projectRoot, ["README.md", "README.rst", "README.txt", "README"])
  if readme:
    content = readFile(readme, maxLines=200)
    context.add("README:\n" + content)

  // Project instructions
  agentMd = findFile(projectRoot, ["NEXUS.md", ".nexus/instructions.md"])
  if agentMd:
    context.add("Project instructions:\n" + readFile(agentMd))

  // Key config files (name + first 50 lines)
  for configFile in ["package.json", "Cargo.toml", "pyproject.toml", "go.mod", "Makefile"]:
    if exists(projectRoot / configFile):
      content = readFile(projectRoot / configFile, maxLines=50)
      context.add(configFile + ":\n" + content)

  return context
```

---

## 6. Token Estimation

Since exact tokenisation depends on the model, the Context Manager uses a fast approximation:

```
function estimateTokens(text):
  // Rough heuristic: 1 token ≈ 4 characters for English text / code.
  // This is intentionally conservative (overestimates) to avoid overflow.
  return ceil(text.length / 3.5)
```

The provider's `usage` chunk (returned after each call) gives exact counts, which are used to calibrate the estimator over the session. If exact counts are available from the previous turn, they take precedence.

---

## 7. Context Window Warning

When `usedTokens / contextWindowSize > 0.80`, the Context Manager sets `warningThreshold = true`. The CLI Interface displays a warning:

```
⚠ Context is 83% full (166,000 / 200,000 tokens). 
  Consider using /clear to start fresh or removing pinned files.
```

---

## 8. Manual File Addition (`/add`)

```
function addFile(filePath):
  resolved = resolveSafePath(filePath, projectRoot)
  content = readFile(resolved)
  tokens = estimateTokens(content)
  
  if tokens > contextWindowSize * 0.25:
    warn("File is very large ({tokens} estimated tokens). Adding first 500 lines.")
    content = readFile(resolved, maxLines=500)
  
  pinnedFiles.add({ path: resolved, content, addedAt: now() })
```

---

## 9. Error Handling

| Case | Behaviour |
|---|---|
| NEXUS.md not found | No error; project instructions section is simply omitted. |
| File added via `/add` doesn't exist | Show error to user. Do not add to context. |
| Context exceeds window after tool results | Trim early conversation history (section 4 priority rules). Log a warning. |
| Token estimation is significantly off | Self-corrects using exact counts from the previous provider response. |

---

## 10. Future Phase Notes

- **Phase 2:** Context compaction replaces the simple "drop oldest turns" strategy with an LLM-generated summary. RAG-based retrieval replaces brute-force auto-gathering with semantic search over an indexed codebase. Persistent memory stores cross-session learnings.
- **Phase 3:** Context assembly respects org policies (e.g., "never include files matching `*.secret`").
