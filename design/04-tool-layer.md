# Niya Component Design: Tool Layer

**Covers Requirements:** TOOL-01 through TOOL-06  
**Status:** Draft  

---

## 1. Purpose

The Tool Layer provides the concrete capabilities the agent uses to interact with the developer's project: reading and writing files, making targeted edits, running shell commands, and searching by file name or content. Each tool is a self-contained unit with a declared schema and an execute function.

---

## 2. Responsibilities

- Implement each tool's logic (file I/O, process spawning, regex search).
- Validate tool inputs against the declared schema before execution.
- Return structured results that are useful to both the model (for reasoning) and the user (for display).
- Enforce project root boundaries — no tool accesses files outside the project root unless explicitly configured.

---

## 3. Tool Interface (shared by all tools)

```
interface Tool {
  schema: ToolSchema
  execute(input: Record<string, any>, context: ToolContext): Promise<ToolResult>
}

interface ToolContext {
  projectRoot: string          // absolute path to the project root
  cwd:         string          // current working directory (usually same as projectRoot)
  env:         Record<string, string>  // environment variables available to shell
}

interface ToolSchema {
  name:        string
  description: string
  parameters:  JSONSchema
}

interface ToolResult {
  success:  boolean
  output:   string                    // text shown to the model
  metadata: Record<string, any>      // structured data for logging / display
}
```

---

## 4. MVP Tools

### 4.1 FileRead

Reads the contents of a file and returns it with line numbers.

**Schema:**
```
name: "file_read"
parameters:
  file_path: string (required) — relative or absolute path
  offset:    number (optional) — line number to start from (0-based)
  limit:     number (optional) — max lines to return (default: 2000)
```

**Behaviour:**
- Resolves `file_path` relative to `projectRoot`.
- Rejects paths outside `projectRoot` (returns error result).
- Returns content prefixed with line numbers (`1\t<line content>`).
- For binary files, returns a message indicating the file is binary and its size.

**Result metadata:** `{ path, lineCount, byteSize, truncated }`

---

### 4.2 FileWrite

Creates a new file or overwrites an existing file.

**Schema:**
```
name: "file_write"
parameters:
  file_path: string (required) — path to create/overwrite
  content:   string (required) — full file content
```

**Behaviour:**
- Creates intermediate directories if they don't exist.
- Rejects paths outside `projectRoot`.
- If the file exists, overwrites it entirely.
- Returns confirmation with the path and byte size.

**Result metadata:** `{ path, byteSize, created: boolean }`

---

### 4.3 FileEdit

Makes a targeted search-and-replace edit to an existing file without rewriting the entire file.

**Schema:**
```
name: "file_edit"
parameters:
  file_path:   string (required) — path to the file
  old_string:  string (required) — exact text to find
  new_string:  string (required) — replacement text
  replace_all: boolean (optional, default: false) — replace all occurrences
```

**Behaviour:**
- Reads the file, finds `old_string`, replaces with `new_string`.
- If `old_string` is not found, returns an error result (not an exception).
- If `old_string` appears multiple times and `replace_all` is false, returns an error asking the model to provide more context for a unique match.
- Writes the modified content back to the file.

**Result metadata:** `{ path, replacements: number, diffPreview: string }`

---

### 4.4 ShellExecute

Runs a shell command and returns stdout, stderr, and exit code.

**Schema:**
```
name: "shell_execute"
parameters:
  command:  string (required) — the shell command to run
  cwd:     string (optional) — working directory (default: projectRoot)
  timeout: number (optional) — timeout in milliseconds (default: 30000)
```

**Behaviour:**
- Spawns a child process with `/bin/sh -c <command>`.
- Captures stdout and stderr separately.
- Kills the process if it exceeds `timeout`.
- Sets environment variables from `ToolContext.env`.
- The output is truncated if it exceeds a configurable limit (default: 100,000 characters) to avoid blowing up the context window.

**Result metadata:** `{ exitCode, stdoutLength, stderrLength, truncated, timedOut }`

**Deny-list:** A configurable list of command patterns that are blocked before execution (e.g., `rm -rf /`, `mkfs`, `dd if=`). See Permission Model for details.

---

### 4.5 Glob

Finds files matching a glob pattern.

**Schema:**
```
name: "glob"
parameters:
  pattern: string (required) — glob pattern (e.g., "src/**/*.ts")
  path:    string (optional) — directory to search in (default: projectRoot)
```

**Behaviour:**
- Searches within `projectRoot` (or the specified `path`, which must be within `projectRoot`).
- Returns matching file paths sorted by modification time (newest first).
- Limits results to 200 entries by default.
- Respects `.gitignore` patterns by default (configurable).

**Result metadata:** `{ matchCount, truncated }`

---

### 4.6 Grep

Searches file contents by regex pattern.

**Schema:**
```
name: "grep"
parameters:
  pattern:     string (required) — regex pattern
  path:        string (optional) — file or directory to search (default: projectRoot)
  glob:        string (optional) — filter files by glob pattern
  context:     number (optional) — lines of context around matches (default: 0)
  max_results: number (optional) — max matches to return (default: 100)
```

**Behaviour:**
- Uses an efficient search implementation (e.g., ripgrep-style engine or recursive regex search).
- Returns matching file paths by default (`files_with_matches` mode).
- With `context > 0`, returns matching lines with surrounding context.
- Respects `.gitignore` by default.

**Result metadata:** `{ matchCount, filesSearched, truncated }`

---

## 5. Tool Registration

At startup, the orchestrator registers all MVP tools with the Tool Registry:

```
registry.register(new FileReadTool())
registry.register(new FileWriteTool())
registry.register(new FileEditTool())
registry.register(new ShellExecuteTool())
registry.register(new GlobTool())
registry.register(new GrepTool())
```

The registry calls `tool.schema` to build the list of schemas sent to the provider.

---

## 6. Path Safety

All file-accessing tools enforce a common path safety rule:

```
function resolveSafePath(inputPath, projectRoot):
  resolved = resolve(projectRoot, inputPath)
  if not resolved.startsWith(projectRoot):
    throw PathOutsideProjectError(inputPath, projectRoot)
  return resolved
```

This prevents directory traversal attacks (e.g., `../../etc/passwd`).

---

## 7. Output Truncation

Tool outputs that exceed the configured character limit are truncated with a notice:

```
"[Output truncated: showing first 100,000 of 523,000 characters. 
 Use file_read with offset/limit to see specific sections.]"
```

This prevents a single tool result from consuming the entire context window.

---

## 8. Future Phase Notes

- **Phase 2:** MCP tools implement the same `Tool` interface and register with the same `ToolRegistry`. Web fetch, web search, and git tools are added. Hooks fire before/after each `tool.execute()` call.
- **Phase 3:** Policy rules can restrict specific tools or tool arguments per user/role.
