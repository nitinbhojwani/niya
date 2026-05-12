# Niya — Requirements Specification

**Version:** 1.0  
**Date:** 2026-05-11  
**Status:** Draft  

---

## 1. Overview

This document specifies the requirements for Niya, a CLI-based coding agent that leverages one or more Large Language Model (LLM) providers — including cloud APIs and locally-running models — to assist developers with code generation, editing, debugging, and project understanding. The agent operates inside the developer's terminal, has access to their project files, and can execute shell commands on their behalf.

The product is designed in three phases. Phase 1 (MVP) targets individual developers with core agentic coding capabilities. Phase 2 adds the integrations, extensibility, and polish needed for daily professional use. Phase 3 introduces team-oriented features and enterprise readiness.

---

## 2. Glossary

| Term | Definition |
|---|---|
| **Niya (the Agent)** | The software system that interprets user intent, plans actions, invokes tools, and produces outputs. |
| **Provider** | An LLM backend (e.g., Anthropic API, OpenAI API, Ollama, llama.cpp) that the agent calls for inference. |
| **Tool** | A capability the agent can invoke: read/write files, run shell commands, search the web, etc. |
| **Context window** | The maximum token budget available for a single inference call to the provider. |
| **Session** | A single continuous interaction between the user and the agent, from start to exit. |
| **Hook** | A user-defined script or command that runs automatically at specific lifecycle events (e.g., before a tool call, after a response). |
| **MCP (Model Context Protocol)** | A protocol for connecting external tool servers to the agent, extending its capabilities. |

---

## 3. Goals and Non-Goals

### 3.1 Goals

- Provide a fast, keyboard-driven interface for agentic coding in the terminal.
- Support multiple LLM providers so users are not locked into a single vendor.
- Allow the agent to read, write, and edit files and execute shell commands within a permission-controlled sandbox.
- Maintain transparency: the user can see and approve every action the agent takes.
- Scale from a single developer's workflow to shared team use over time.

### 3.2 Non-Goals

- Building or fine-tuning a foundation model. The agent consumes existing model APIs.
- Replacing a full IDE. The agent complements editors and IDEs, not replaces them.
- Providing a graphical user interface (GUI) in Phase 1. GUI/IDE integrations are deferred to Phase 2+.

---

## 4. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                      CLI Interface                      │
├─────────────────────────────────────────────────────────┤
│                   Agent Orchestrator                    │
│  ┌──────────┐  ┌──────────────┐  ┌───────────────────┐ │
│  │  Planner  │  │ Tool Router  │  │ Context Manager   │ │
│  └──────────┘  └──────────────┘  └───────────────────┘ │
├─────────────────────────────────────────────────────────┤
│                  Provider Abstraction                   │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────────────┐  │
│  │Claude  │ │OpenAI  │ │Ollama  │ │ Custom / Local │  │
│  └────────┘ └────────┘ └────────┘ └────────────────┘  │
├─────────────────────────────────────────────────────────┤
│                     Tool Layer                          │
│  File R/W │ Shell Exec │ Search │ MCP Servers │ Hooks  │
└─────────────────────────────────────────────────────────┘
```

The system is composed of four layers: the CLI interface that the user interacts with, the agent orchestrator that plans and routes actions, the provider abstraction that normalises communication with different LLM backends, and the tool layer that provides concrete capabilities.

---

## 5. Phased Requirements

---

### Phase 1 — MVP (Minimum Viable Product)

**Objective:** A working CLI agent that a single developer can install, configure with at least one LLM provider, and use for everyday coding tasks — file edits, code generation, debugging, and project Q&A.

#### 5.1.1 CLI Interface

| ID | Requirement | Priority |
|---|---|---|
| CLI-01 | The agent launches from a single command (e.g., `niya`) in any terminal emulator on macOS and Linux. | Must |
| CLI-02 | The agent accepts a natural-language prompt as a positional argument for non-interactive use (e.g., `niya "fix the failing tests"`). | Must |
| CLI-03 | The agent supports an interactive REPL mode with streaming token output. | Must |
| CLI-04 | The REPL supports common line-editing shortcuts (arrow keys, Ctrl-C to cancel, Ctrl-D to exit). | Must |
| CLI-05 | The agent can read from stdin/pipe (e.g., `git diff | niya "review this diff"`). | Should |
| CLI-06 | The agent displays a clear distinction between its own reasoning/output and tool invocations (e.g., file edits, shell commands). | Must |
| CLI-07 | The agent renders markdown in terminal output (bold, code blocks, headings) for readability. | Should |

#### 5.1.2 Provider Abstraction

| ID | Requirement | Priority |
|---|---|---|
| PROV-01 | The agent supports at least two cloud LLM providers (e.g., Anthropic Claude, OpenAI GPT) at launch. | Must |
| PROV-02 | The agent supports at least one local model runtime (e.g., Ollama, llama.cpp) at launch. | Must |
| PROV-03 | Provider configuration (API keys, base URLs, model names) is stored in a user-level config file (e.g., `~/.niya/config.yaml`). | Must |
| PROV-04 | The user can switch providers/models via a CLI flag (`--provider`, `--model`) or config default. | Must |
| PROV-05 | The provider abstraction normalises request/response formats so the orchestrator is provider-agnostic. | Must |
| PROV-06 | API key validation occurs at startup with a clear error message on failure. | Must |
| PROV-07 | The agent handles provider rate-limits gracefully with exponential backoff and user notification. | Should |

#### 5.1.3 Core Tools

| ID | Requirement | Priority |
|---|---|---|
| TOOL-01 | **File Read:** The agent can read any file in the project directory tree. | Must |
| TOOL-02 | **File Write:** The agent can create new files and overwrite existing files. | Must |
| TOOL-03 | **File Edit:** The agent can make targeted edits (search-and-replace) to existing files without rewriting the entire file. | Must |
| TOOL-04 | **Shell Execute:** The agent can run shell commands and capture stdout/stderr. | Must |
| TOOL-05 | **Glob/Find:** The agent can search for files by name pattern. | Must |
| TOOL-06 | **Grep/Search:** The agent can search file contents by regex. | Must |
| TOOL-07 | All tool invocations are displayed to the user before execution. The user can configure auto-approve rules or require manual approval per tool type. | Must |

#### 5.1.4 Permission and Safety Model

| ID | Requirement | Priority |
|---|---|---|
| PERM-01 | The agent operates within a defined project root and cannot access files outside it without explicit configuration. | Must |
| PERM-02 | Shell commands run in a controlled environment. Destructive commands (e.g., `rm -rf /`) are blocked by a deny-list. | Must |
| PERM-03 | The user can configure per-tool permission levels: `ask` (prompt before every call), `auto` (always allow), `deny` (never allow). | Must |
| PERM-04 | The agent logs all tool invocations (tool name, arguments, result summary) to a session log file. | Must |
| PERM-05 | A `--dry-run` flag previews all planned actions without executing them. | Should |

#### 5.1.5 Context Management

| ID | Requirement | Priority |
|---|---|---|
| CTX-01 | The agent automatically includes relevant project context (e.g., README, directory tree, recent edits) in the prompt. | Must |
| CTX-02 | The agent tracks token usage and warns the user when approaching the provider's context window limit. | Must |
| CTX-03 | The agent supports a conversation history within a session, enabling multi-turn interactions. | Must |
| CTX-04 | The user can manually add files or URLs to the context via commands (e.g., `/add src/main.py`). | Should |
| CTX-05 | The agent supports a project-level instruction file (e.g., `NIYA.md`) that is automatically loaded into context at session start. | Should |

#### 5.1.6 Installation and Configuration

| ID | Requirement | Priority |
|---|---|---|
| INST-01 | The agent is installable via npm (`npm install -g @niya/cli`) and as a standalone binary (no runtime dependency). | Must |
| INST-02 | First-run setup guides the user through provider configuration interactively. | Should |
| INST-03 | Configuration supports both global (`~/.niya/`) and project-level (`.niya/`) settings, with project-level taking precedence. | Must |

#### Phase 1 — Acceptance Criteria

1. A developer can install the agent, configure one LLM provider, and start an interactive session in under 5 minutes.
2. The agent can read a codebase, answer questions about it, generate new files, and edit existing files with targeted diffs.
3. The agent can run shell commands (tests, linters, build scripts) and react to their output.
4. All file and shell operations are logged and can be audited.
5. The agent works with at least two cloud providers and one local model runtime.

---

### Phase 2 — Core Features and Integrations

**Objective:** Make the agent a power tool for daily professional development — extensible via plugins and MCP servers, integrated with IDEs and version control, with robust context management for large codebases.

#### 5.2.1 Extensibility

| ID | Requirement | Priority |
|---|---|---|
| EXT-01 | The agent supports MCP (Model Context Protocol) servers, allowing external tools (databases, APIs, documentation sources) to be connected. | Must |
| EXT-02 | Users can install and manage MCP servers via CLI commands (e.g., `niya mcp add`, `niya mcp list`). | Must |
| EXT-03 | The agent supports user-defined hooks that execute at lifecycle events: pre-tool-call, post-tool-call, session-start, session-end, notification. | Must |
| EXT-04 | The agent supports slash commands (e.g., `/review`, `/test`, `/commit`) that map to predefined or user-defined workflows. | Should |
| EXT-05 | A plugin/skill system allows packaging and distributing reusable agent behaviours (prompts + tools + hooks). | Should |

#### 5.2.2 Advanced Context Management

| ID | Requirement | Priority |
|---|---|---|
| ACTX-01 | The agent supports automatic context compaction/summarisation when the conversation approaches the context limit, allowing very long sessions without manual intervention. | Must |
| ACTX-02 | The agent maintains a persistent memory store across sessions (e.g., project conventions, user preferences, learnings) that it can query. | Should |
| ACTX-03 | The agent supports indexing large codebases and retrieving relevant code via semantic search (RAG). | Should |
| ACTX-04 | The agent can follow references across files (e.g., "find all callers of this function") using language-aware analysis or tree-sitter. | Should |

#### 5.2.3 Version Control Integration

| ID | Requirement | Priority |
|---|---|---|
| VCS-01 | The agent understands git context: current branch, uncommitted changes, recent commits. | Must |
| VCS-02 | The agent can create commits with well-formatted messages summarising changes it made. | Must |
| VCS-03 | The agent can create, switch, and manage branches. | Should |
| VCS-04 | The agent can generate pull request descriptions from a diff. | Should |
| VCS-05 | The agent supports a "worktree" mode that isolates its changes in a separate git worktree, preventing interference with the user's working tree. | Should |

#### 5.2.4 IDE and Editor Integration

| ID | Requirement | Priority |
|---|---|---|
| IDE-01 | The agent provides a VS Code extension that embeds the agent in the editor's sidebar or terminal panel. | Must |
| IDE-02 | The IDE integration supports inline diff previews for proposed file changes. | Should |
| IDE-03 | The agent provides a JetBrains plugin. | Could |
| IDE-04 | The agent provides a Vim/Neovim plugin. | Could |

#### 5.2.5 Web and Documentation Access

| ID | Requirement | Priority |
|---|---|---|
| WEB-01 | The agent can fetch and read web pages (documentation, Stack Overflow, API references) when given a URL. | Must |
| WEB-02 | The agent can perform web searches to find relevant information. | Should |
| WEB-03 | Fetched web content respects robots.txt and rate limits. | Must |

#### 5.2.6 Multi-Step Agentic Workflows

| ID | Requirement | Priority |
|---|---|---|
| WKFL-01 | The agent supports autonomous multi-step workflows: plan a sequence of actions, execute them, and self-correct on failure (agentic loop). | Must |
| WKFL-02 | The user can interrupt a running workflow at any point (Ctrl-C or Escape) and resume or modify the plan. | Must |
| WKFL-03 | The agent can spawn sub-agents for parallel or independent sub-tasks. | Should |
| WKFL-04 | The agent implements a "plan mode" that proposes a full plan for user approval before execution. | Should |

#### Phase 2 — Acceptance Criteria

1. A developer can connect at least one MCP server and use it within a coding session.
2. The agent can work on a codebase with 100k+ lines of code, using indexing or RAG to stay relevant.
3. The agent can make a series of changes, run tests, and iterate until tests pass — autonomously.
4. The agent integrates into VS Code with inline diff preview.
5. Hooks and slash commands are configurable and documented.

---

### Phase 3 — Team Collaboration and Enterprise

**Objective:** Enable teams to share configurations, enforce policies, manage costs, and use the agent within enterprise-grade security and compliance boundaries.

#### 5.3.1 Team Configuration and Sharing

| ID | Requirement | Priority |
|---|---|---|
| TEAM-01 | Teams can define shared agent configurations (approved providers, default models, custom instructions) in a version-controlled team config file. | Must |
| TEAM-02 | Organisation-level settings can override or restrict individual user settings (e.g., enforce a specific provider, block shell access). | Must |
| TEAM-03 | Custom slash commands and skills can be published to a team-internal registry/marketplace. | Should |
| TEAM-04 | Teams can share and version prompt templates for common tasks (code review, PR description, commit message). | Should |

#### 5.3.2 Authentication and Access Control

| ID | Requirement | Priority |
|---|---|---|
| AUTH-01 | The agent supports SSO/SAML authentication for enterprise identity providers. | Must |
| AUTH-02 | Role-based access control (RBAC) governs which tools, providers, and capabilities are available to each user or group. | Must |
| AUTH-03 | API key management supports scoped, rotatable tokens with audit trails. | Must |
| AUTH-04 | The agent supports running in air-gapped environments using only local models (no external network calls). | Should |

#### 5.3.3 Audit, Compliance, and Observability

| ID | Requirement | Priority |
|---|---|---|
| AUDIT-01 | All agent sessions are logged with: user identity, prompts sent, tools invoked, files modified, commands executed. | Must |
| AUDIT-02 | Logs can be shipped to external systems (SIEM, log aggregators) via standard formats (JSON, OpenTelemetry). | Must |
| AUDIT-03 | Administrators can define policy rules that restrict agent behaviour (e.g., "never modify files in /prod", "block network-accessing tools"). | Must |
| AUDIT-04 | The agent provides usage analytics: token consumption, cost estimates per user/team/project, and model usage breakdowns. | Should |
| AUDIT-05 | Session recordings can be replayed for review or training purposes. | Could |

#### 5.3.4 Cost Management

| ID | Requirement | Priority |
|---|---|---|
| COST-01 | The agent tracks and reports per-session token usage and estimated cost (based on provider pricing). | Must |
| COST-02 | Administrators can set usage quotas (daily/monthly token limits) per user or team. | Must |
| COST-03 | The agent supports routing: automatically selecting cheaper models for simpler tasks and more capable models for complex ones. | Should |

#### 5.3.5 Deployment and Distribution

| ID | Requirement | Priority |
|---|---|---|
| DEPLOY-01 | The agent can be deployed as a centralised service (e.g., a shared server) that team members connect to, in addition to local CLI installation. | Should |
| DEPLOY-02 | The agent supports containerised deployment (Docker image) for consistent environments. | Must |
| DEPLOY-03 | Automatic updates with rollback capability. | Should |

#### Phase 3 — Acceptance Criteria

1. An engineering team of 10+ developers can share a common agent configuration with enforced provider policies.
2. An admin can view a dashboard of agent usage across the team, including cost breakdowns.
3. All agent activity is auditable and exportable to enterprise logging systems.
4. The agent authenticates via the organisation's SSO provider.
5. Usage quotas prevent runaway costs.

---

## 6. Non-Functional Requirements

| ID | Requirement | Phase |
|---|---|---|
| NFR-01 | **Latency:** Time-to-first-token in interactive mode must be under 2 seconds for cloud providers on a standard broadband connection. | 1 |
| NFR-02 | **Reliability:** The agent must handle provider outages gracefully — retry, fall back to an alternative provider, or inform the user clearly. | 1 |
| NFR-03 | **Security:** No user code or conversation data is stored or transmitted beyond the configured provider's API. The agent itself does not phone home. | 1 |
| NFR-04 | **Privacy:** Credentials and API keys are stored securely (OS keychain or encrypted config) and never logged in plaintext. | 1 |
| NFR-05 | **Portability:** The agent runs on macOS (ARM and Intel), Linux (x86_64 and ARM64), and Windows (via WSL in Phase 1, native in Phase 2+). | 1–2 |
| NFR-06 | **Performance:** The agent should handle projects with up to 1 million lines of code without degraded responsiveness for file search and navigation. | 2 |
| NFR-07 | **Extensibility:** All core tools are implemented as internal MCP servers, eating our own dog food and ensuring third-party tools are first-class citizens. | 2 |
| NFR-08 | **Accessibility:** CLI output supports screen readers and high-contrast terminal themes. | 2 |
| NFR-09 | **Scalability:** The centralised deployment (Phase 3) supports at least 100 concurrent users per instance. | 3 |
| NFR-10 | **Error Handling:** The agent provides clear, actionable error messages for all failure modes — provider errors, file permission issues, malformed input, and partial tool failures mid-workflow. It recovers gracefully rather than crashing. | 1 |
| NFR-11 | **Documentation and Help:** The agent ships with `--help` for all commands and sub-commands, an in-session `/help` command, and hosted user documentation. | 1 |
| NFR-12 | **Self-Update:** The CLI supports a self-update mechanism (e.g., `niya update`) that checks for and installs new versions with rollback on failure. | 2 |
| NFR-13 | **Testing and CI:** The project maintains automated test suites — unit tests for core logic, integration tests per provider, and end-to-end tests for common workflows. CI runs on every commit. | 1 |

---

## 7. Risks and Mitigations

| Risk | Impact | Likelihood | Mitigation |
|---|---|---|---|
| LLM provider API changes break the agent | High | Medium | Abstraction layer isolates provider-specific logic; automated integration tests per provider. |
| Agent executes destructive shell commands | Critical | Low | Permission model with deny-lists, sandboxed execution, and mandatory logging. |
| Context window limits degrade quality on large codebases | High | High | RAG-based retrieval (Phase 2), automatic context compaction, and chunked processing. |
| Local model quality is significantly worse than cloud models | Medium | High | Clear documentation of model capabilities; allow users to mix local (fast/cheap) and cloud (quality) models. |
| Cost overruns from uncontrolled token usage | Medium | Medium | Per-session cost tracking (Phase 1), quotas and routing (Phase 3). |
| Security concerns block enterprise adoption | High | Medium | Audit logging, air-gapped mode, SSO, and RBAC (Phase 3). |

---

## 8. Open Questions

1. **Streaming protocol for local models:** Should the agent standardise on OpenAI-compatible API format for local models, or support multiple local protocols?
2. **State persistence:** How much session state should persist between agent invocations — just memory, or also undo history and conversation?
3. **Multi-language support:** Should the agent's UI be internationalised, or is English-only acceptable through Phase 2?
4. **Licensing model:** Open source (core) with commercial enterprise tier, or fully commercial?
5. **Telemetry:** Should anonymised usage telemetry be collected to improve the product? If so, what opt-in/opt-out model?

---

## 9. Phase Summary

| Aspect | Phase 1 (MVP) | Phase 2 (Core) | Phase 3 (Enterprise) |
|---|---|---|---|
| **Users** | Individual developers | Power users, small teams | Engineering orgs, enterprise |
| **Providers** | 2 cloud + 1 local | + seamless switching, routing hints | + cost-based routing, quotas |
| **Tools** | File R/W, edit, shell, search | + MCP, hooks, slash commands, web | + policy-restricted tools |
| **Context** | Basic conversation, manual add | RAG, compaction, persistent memory | Same, at scale |
| **VCS** | None | Git-aware, commits, PRs | Same |
| **IDE** | Terminal only | VS Code, JetBrains, Vim | Same |
| **Auth** | API keys in config | Same | SSO, RBAC, scoped tokens |
| **Audit** | Local session logs | Same | Centralised logging, SIEM export |
| **Cost** | Per-session display | Same | Quotas, dashboards, routing |
| **Deployment** | Local CLI | Same | + centralised server, Docker |

---

*End of specification.*
