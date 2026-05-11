# Nexus — Architecture Overview

**Scope:** Phase 1 (MVP)  
**Date:** 2026-05-11  
**Status:** Draft  

---

## 1. Introduction

This document describes the high-level architecture of Nexus's MVP. It identifies the major components, their responsibilities, how they interact, and the key design decisions that shape the system.

The MVP delivers Nexus, a CLI-based coding agent that connects to multiple LLM providers (cloud and local), reads and modifies project files, executes shell commands, and manages conversational context — all within a permission-controlled environment.

---

## 2. Design Principles

1. **Provider-agnostic core.** The orchestrator and tools never depend on a specific LLM provider. All provider-specific logic is isolated behind a uniform interface.
2. **Tools as the only side-effect boundary.** The agent reasons and plans in pure logic; every interaction with the outside world (file system, shell, network) goes through a registered tool with a declared schema.
3. **User stays in control.** Every tool invocation is subject to the permission model. The user can audit, approve, or block any action.
4. **Minimal abstraction.** Favour simple, composable interfaces over deep inheritance hierarchies. Each component should be replaceable independently.
5. **Offline-capable.** When configured with a local model provider, the agent must function without any network access.

---

## 3. Component Map

```
┌──────────────────────────────────────────────────────────────┐
│                        CLI Interface                         │
│         (input parsing, REPL, output rendering)              │
└──────────────────────┬───────────────────────────────────────┘
                       │ user messages, commands
                       ▼
┌──────────────────────────────────────────────────────────────┐
│                    Agent Orchestrator                         │
│  ┌───────────────┐ ┌──────────────┐ ┌─────────────────────┐ │
│  │    Planner     │ │  Tool Router │ │  Context Manager    │ │
│  │ (agentic loop) │ │  (dispatch)  │ │  (window, history)  │ │
│  └───────┬───────┘ └──────┬───────┘ └──────────┬──────────┘ │
│          │                │                     │            │
│          └────────────────┼─────────────────────┘            │
│                           │                                  │
│  ┌────────────────────────┴────────────────────────────────┐ │
│  │                  Permission Gate                        │ │
│  │          (approve / deny / ask user)                    │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────┬───────────────────────────────────────┘
                       │ tool calls (post-permission)
                       ▼
┌──────────────────────────────────────────────────────────────┐
│                      Provider Layer                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Claude   │  │ OpenAI   │  │ Ollama   │  │ Custom     │  │
│  │ Adapter  │  │ Adapter  │  │ Adapter  │  │ Adapter    │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                       Tool Layer                             │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐ │
│  │ File     │ │ Shell    │ │ Search   │ │ Session Log    │ │
│  │ Tools    │ │ Executor │ │ Tools    │ │                │ │
│  └──────────┘ └──────────┘ └──────────┘ └────────────────┘ │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                   Configuration System                       │
│           (global, project-level, CLI overrides)             │
└──────────────────────────────────────────────────────────────┘
```

---

## 4. Component Responsibilities

| Component | Responsibility | Detailed Design |
|---|---|---|
| **CLI Interface** | Parses arguments, runs the REPL, streams output, renders markdown, handles signals (Ctrl-C, Ctrl-D). | [01-cli-interface.md](./01-cli-interface.md) |
| **Agent Orchestrator** | The central loop: receives user input, invokes the provider for planning/reasoning, dispatches tool calls, and returns results. Contains the Planner, Tool Router, and Context Manager as sub-components. | [02-agent-orchestrator.md](./02-agent-orchestrator.md) |
| **Provider Layer** | Normalises communication with LLM backends. Each adapter translates between the internal message format and the provider's API. | [03-provider-abstraction.md](./03-provider-abstraction.md) |
| **Tool Layer** | Implements concrete capabilities: file read/write/edit, shell execution, glob, grep. Each tool declares a schema and an execute function. | [04-tool-layer.md](./04-tool-layer.md) |
| **Permission Model** | Gates every tool invocation. Evaluates the configured permission level for the tool, prompts the user if needed, and logs the decision. | [05-permission-model.md](./05-permission-model.md) |
| **Context Manager** | Assembles the prompt sent to the provider: system instructions, project context, conversation history, and tool results. Tracks token budget. | [06-context-manager.md](./06-context-manager.md) |
| **Configuration System** | Loads and merges config from global, project, and CLI sources. Validates schemas. Provides typed access to all settings. | [07-configuration.md](./07-configuration.md) |

---

## 5. Data Flow

### 5.1 Single Turn (User → Response)

```
1. User types a message in the REPL (CLI Interface)
2. CLI Interface passes the message to the Agent Orchestrator
3. Context Manager assembles the full prompt:
   - System prompt + project instructions (NEXUS.md)
   - Conversation history
   - Available tool schemas
   - User message
4. Orchestrator sends the prompt to the active Provider Adapter
5. Provider streams back a response (text + tool_use blocks)
6. For each tool_use block:
   a. Tool Router resolves the tool by name
   b. Permission Gate checks policy for this tool
      - If "ask": CLI Interface prompts the user → approve/deny
      - If "auto": proceed
      - If "deny": return an error to the model
   c. Tool executes and returns a result
   d. Session Log records the invocation
   e. Result is appended to conversation and sent back to Provider
7. Provider returns a final text response (no more tool calls)
8. CLI Interface streams the response to the terminal
9. Context Manager appends the full turn to conversation history
```

### 5.2 Multi-Turn Loop

The orchestrator repeats steps 4–9 in a loop. The provider decides when to stop calling tools and emit a final response. The loop has a configurable maximum iteration count (default: 20) as a safety valve.

---

## 6. Key Design Decisions

### 6.1 Streaming-First

All provider responses are streamed token-by-token to the terminal. This gives immediate feedback and avoids the user staring at a blank screen. Tool call blocks are detected as they arrive and rendered distinctly (e.g., with a spinner and tool name).

### 6.2 Tool Schemas as the Contract

Every tool is defined by a schema (name, description, input parameters with types, output type). This schema serves three purposes: it is sent to the LLM so it knows what tools are available; it is used by the Permission Gate to identify the tool; and it is used for input validation before execution.

### 6.3 Provider Adapters Are Thin

Adapters do only three things: translate the internal message format to the provider's API format, manage the HTTP/gRPC connection, and parse the streaming response back into the internal format. No business logic lives in an adapter.

### 6.4 Configuration Layering

Settings are merged with a clear precedence: CLI flags > environment variables > project config (`.nexus/config.yaml`) > global config (`~/.nexus/config.yaml`) > defaults. Each layer is optional.

### 6.5 Session Logging as a Core Feature

Every tool invocation, permission decision, and provider call is logged to a structured (JSON lines) session log. This is not an afterthought — it is a core component that the audit and replay features in Phase 3 will build upon.

---

## 7. Extension Points for Future Phases

While this design targets the MVP, these seams are intentionally built in:

| Extension Point | Future Phase | How It's Prepared |
|---|---|---|
| **MCP Servers** | Phase 2 | Tools already use a schema-based interface. MCP tools will implement the same interface and register with the Tool Router. |
| **Hooks** | Phase 2 | The orchestrator loop has named lifecycle events (pre-tool, post-tool, session-start, session-end). Hook registration will attach to these. |
| **RAG / Indexing** | Phase 2 | The Context Manager's `gatherProjectContext()` method is the natural place to add retrieval-augmented generation. |
| **IDE Integration** | Phase 2 | The CLI Interface is a thin layer. An IDE integration replaces this layer with a language server / extension, keeping the orchestrator unchanged. |
| **RBAC / Policies** | Phase 3 | The Permission Gate already makes per-tool decisions. Org-level policies will feed into the same gate as additional rules. |
| **Centralised Deployment** | Phase 3 | The orchestrator is stateless per-session. Wrapping it in a server (HTTP/WebSocket) is straightforward. |

---

## 8. Directory Structure (Design Docs)

```
design/
├── 00-architecture-overview.md    ← this document
├── 01-cli-interface.md
├── 02-agent-orchestrator.md
├── 03-provider-abstraction.md
├── 04-tool-layer.md
├── 05-permission-model.md
├── 06-context-manager.md
└── 07-configuration.md
```

---

*Next: individual component designs.*
