# Decision Record: Tech Stack Selection

**Decision ID:** DR-001  
**Status:** Open  
**Date:** 2026-05-11  
**Context:** Choose the primary language and runtime for the coding agent CLI, orchestrator, and tool layer.  

---

## 1. Decision Drivers

The tech stack must support the following well:

- **Streaming I/O** — token-by-token streaming from LLM provider APIs to the terminal.
- **Process management** — spawning child processes (shell commands), capturing stdout/stderr, enforcing timeouts.
- **Async concurrency** — multiple in-flight operations (provider stream + user input + tool execution) without blocking.
- **Fast startup** — the CLI should feel instant; sub-200ms to first prompt.
- **Cross-platform distribution** — single binary or low-friction install on macOS, Linux, and Windows.
- **Ecosystem for LLM integrations** — mature SDK support for Anthropic, OpenAI, Ollama, and OpenAI-compatible APIs.
- **Developer productivity** — the team's ability to iterate quickly on the MVP.
- **Community and extensibility** — users and contributors should be able to write plugins, hooks, and MCP servers easily.

---

## 2. Options Evaluated

### Option A: TypeScript / Node.js

The approach taken by Claude Code. TypeScript on the Node.js runtime, distributed via npm and optionally compiled to a standalone binary with a bundler (e.g., pkg, bun compile, or esbuild + sea).

| Dimension | Assessment |
|---|---|
| Streaming I/O | Excellent. Native async iterators, `fetch` with streaming, SSE libraries are mature. Node streams are first-class. |
| Process management | Excellent. `child_process.spawn` with stdio piping is well-proven. Libraries like `execa` add timeout, signal handling, and buffering. |
| Async concurrency | Excellent. Event loop + `async/await` handles concurrent streams naturally. No thread management needed. |
| Startup time | Moderate. Node cold-start is ~100–300ms. Bundling to a single JS file helps. Bun reduces this further (~50ms). |
| Cross-platform distribution | Good. npm install works everywhere Node runs. Standalone binaries via pkg/bun add ~50MB to the download but remove the Node dependency. |
| LLM SDK ecosystem | Excellent. Official Anthropic and OpenAI SDKs are TypeScript-first. Ollama has a well-maintained JS client. Most LLM tooling targets JS/TS first. |
| Developer productivity | High. Large talent pool, fast iteration, strong typing with TypeScript, rich package ecosystem. |
| Plugin/extension ecosystem | Excellent. npm is the largest package registry. MCP protocol has a reference implementation in TypeScript. Users are likely to know JS/TS. |
| Terminal UI | Good. Libraries like `ink` (React for CLI), `chalk`, `ora` (spinners), `marked-terminal` are mature. |
| Memory usage | Moderate. Node processes typically use 50–150MB baseline. Acceptable for a CLI tool but not lightweight. |
| Binary size | Moderate–Large. Standalone binaries are 50–80MB due to bundled V8 engine. |

**Pros:**
- Proven at scale by Claude Code — the exact same problem domain.
- Fastest path to MVP: the richest ecosystem for LLM integrations, terminal UI, and async I/O.
- TypeScript's type system catches interface mismatches at compile time.
- MCP reference implementation is in TypeScript, so MCP integration is trivial.
- Largest pool of potential contributors.

**Cons:**
- Node.js startup is slower than compiled languages.
- Standalone binary is large (~50–80MB) due to bundled V8.
- Node's memory footprint is higher than Go or Rust.
- npm dependency tree can be a supply-chain risk surface.

---

### Option B: Python

A Python CLI built with a framework like Typer or Click, using asyncio for concurrency.

| Dimension | Assessment |
|---|---|
| Streaming I/O | Good. `httpx` and `aiohttp` support streaming. `asyncio` handles async iteration. Less ergonomic than Node's native streams. |
| Process management | Good. `asyncio.create_subprocess_exec` works but is more verbose than Node equivalents. |
| Async concurrency | Adequate. asyncio works but has a steeper learning curve, and mixing sync/async code is error-prone (the "colour problem"). |
| Startup time | Slow. Python cold-start is 200–500ms. Import-heavy apps can exceed 1 second. |
| Cross-platform distribution | Poor–Moderate. PyInstaller and Nuitka produce standalone binaries but are fragile across OS versions. pip install requires a Python runtime. Users may have version conflicts. |
| LLM SDK ecosystem | Good. Official Anthropic and OpenAI SDKs exist in Python. Ollama has a Python client. LangChain and related tooling are Python-first. |
| Developer productivity | High for prototyping, moderate for maintaining a large CLI. No compile-time type safety (mypy is optional and incomplete). |
| Plugin/extension ecosystem | Good. pip/PyPI is large. However, Python's packaging story (venv, pip, conda, poetry) is fragmented and can frustrate users. |
| Terminal UI | Moderate. `rich` library is excellent for formatting. `prompt_toolkit` for REPL. Fewer integrated CLI frameworks than Node. |
| Memory usage | Moderate. Similar to Node (~50–100MB for a typical process). |
| Binary size | Large. PyInstaller binaries are 80–150MB. |

**Pros:**
- Many ML/AI developers are already in the Python ecosystem.
- Excellent libraries for text processing, regex, and data manipulation.
- `rich` library produces beautiful terminal output with minimal effort.
- LangChain, LlamaIndex, and other AI orchestration frameworks are Python-native.

**Cons:**
- Distribution is Python's weakest point. Managing Python versions and virtual environments is a notorious pain point for end-user CLI tools.
- Slow startup hurts the "feels instant" requirement.
- asyncio's sync/async boundary creates friction in a heavily concurrent application.
- No compile-time type safety makes refactoring the provider abstraction and tool interfaces riskier.
- Standalone binaries are large and brittle.

---

### Option C: Rust

A Rust CLI, leveraging `tokio` for async runtime, `clap` for argument parsing, and direct HTTP calls for provider APIs.

| Dimension | Assessment |
|---|---|
| Streaming I/O | Excellent. `tokio` + `reqwest` support streaming. `futures::Stream` is powerful. |
| Process management | Excellent. `tokio::process::Command` with full async support. |
| Async concurrency | Excellent. `tokio` is industrial-strength. Ownership model prevents data races at compile time. |
| Startup time | Excellent. Native binary, near-zero startup (<10ms). |
| Cross-platform distribution | Excellent. Single static binary, no runtime dependency. Cross-compilation with `cross` is well-supported. Small binaries (5–15MB). |
| LLM SDK ecosystem | Poor. No official Anthropic or OpenAI Rust SDKs. Community crates exist but are less maintained. You'd likely write your own HTTP client wrappers. |
| Developer productivity | Low–Moderate. Rust's learning curve is steep. Borrow checker fights are common in async code with shared state. Iteration speed is slower than TS or Python. |
| Plugin/extension ecosystem | Poor. crates.io is small relative to npm/PyPI. Users writing Rust plugins is a high barrier. WASM plugins are an option but add complexity. |
| Terminal UI | Good. `ratatui`, `crossterm`, `indicatif` are solid. Less polished than Node/Python equivalents for markdown rendering. |
| Memory usage | Excellent. Minimal memory footprint (10–30MB). |
| Binary size | Excellent. 5–15MB for a feature-rich CLI. |

**Pros:**
- Best-in-class startup time, binary size, and memory usage.
- Single static binary with zero dependencies — the ideal distribution story.
- Memory safety and concurrency guarantees eliminate entire classes of bugs.
- Perceived as a serious, high-quality tool by the developer community.

**Cons:**
- Significantly slower development velocity, especially for the MVP phase.
- Weak LLM SDK ecosystem means building and maintaining provider adapters from scratch.
- High barrier for community contributions and plugin development.
- Rust's async ecosystem, while powerful, has sharp edges (pinning, lifetimes in async contexts).
- Harder to prototype and iterate on prompt engineering and agent logic.

---

### Option D: Go

A Go CLI, leveraging goroutines for concurrency, distributed as a single static binary.

| Dimension | Assessment |
|---|---|
| Streaming I/O | Good. `net/http` supports streaming. SSE parsing requires a small library or manual implementation. |
| Process management | Good. `os/exec` is straightforward. Goroutines make concurrent process management easy. |
| Async concurrency | Good. Goroutines and channels are simple and effective. No async/await complexity. |
| Startup time | Excellent. Native binary, startup is <20ms. |
| Cross-platform distribution | Excellent. Single static binary. `GOOS`/`GOARCH` cross-compilation is trivial. Binaries are 10–25MB. |
| LLM SDK ecosystem | Poor–Moderate. No official Anthropic SDK. OpenAI has a community Go client. Ollama's own CLI is written in Go, so that integration is natural. |
| Developer productivity | Moderate. Go is simple to learn and fast to compile. However, the lack of generics (pre-1.18 style) and verbose error handling slow down complex type abstractions. |
| Plugin/extension ecosystem | Poor. Go's plugin system (`plugin` package) is Linux-only and fragile. hashicorp/go-plugin (gRPC-based) is the practical alternative but adds complexity. |
| Terminal UI | Moderate. `cobra` for CLI framework, `bubbletea`/`lipgloss` for TUI, `glamour` for markdown rendering. Ecosystem is growing but less mature than Node. |
| Memory usage | Good. 20–50MB typical for a CLI process. |
| Binary size | Good. 10–25MB for a typical CLI. |

**Pros:**
- Simple language with fast compilation — good iteration speed for a systems-level tool.
- Excellent distribution story (single binary, trivial cross-compilation).
- Goroutine model is a natural fit for concurrent streaming + tool execution.
- Ollama is written in Go, so deep integration with local models could reuse Ollama's internals.

**Cons:**
- Weak LLM SDK ecosystem — most provider integrations would need custom HTTP clients.
- Go's type system is less expressive than TypeScript for modelling the provider abstraction and tool schemas (no union types, limited generics).
- Plugin ecosystem is immature. Extending the agent with user-defined tools is harder.
- Markdown rendering and terminal UI libraries are less polished than Node equivalents.
- Smaller overlap with the target user base (developers using LLM coding tools tend to be JS/Python-heavy).

---

### Option E: Hybrid — TypeScript Core + Rust Performance Layer

Use TypeScript/Node.js for the orchestrator, CLI, provider adapters, and plugin system. Use Rust (compiled to native addon via NAPI-RS, or to WASM) for performance-critical operations: file search (grep/glob), large file parsing, and token estimation.

| Dimension | Assessment |
|---|---|
| Streaming I/O | Excellent (TypeScript layer). |
| Process management | Excellent (TypeScript layer). |
| Startup time | Moderate. Same as Option A (~100–300ms) since the Node runtime still loads. |
| Cross-platform distribution | Moderate. Native addons require per-platform prebuilt binaries. NAPI-RS handles this but adds CI complexity. |
| LLM SDK ecosystem | Excellent (TypeScript layer). |
| Developer productivity | High for most code (TypeScript). Lower for the Rust boundary — requires two build systems and two skill sets. |
| Performance-critical paths | Excellent. Rust grep/glob can be orders of magnitude faster than pure JS equivalents on large codebases. |
| Plugin ecosystem | Excellent (npm/TypeScript). Plugins don't need to touch Rust. |

**Pros:**
- Best of both worlds: fast development in TypeScript, native performance where it matters.
- The Rust layer is small and well-bounded (search, parsing), reducing the Rust expertise needed.
- This is the pattern used by tools like SWC, Turbopack, and Biome.

**Cons:**
- Two build systems, two languages, more CI complexity.
- Native addon distribution across platforms is a known pain point (though NAPI-RS has improved this significantly).
- Harder to onboard contributors who need to work across the boundary.
- May be premature optimisation for the MVP — pure TypeScript may be fast enough initially.

---

## 3. Comparison Matrix (General)

| Criterion | Weight | TypeScript | Python | Rust | Go | Hybrid (TS+Rust) |
|---|---|---|---|---|---|---|
| Streaming & async I/O | High | ★★★★★ | ★★★☆☆ | ★★★★★ | ★★★★☆ | ★★★★★ |
| Startup time | Medium | ★★★☆☆ | ★★☆☆☆ | ★★★★★ | ★★★★★ | ★★★☆☆ |
| Cross-platform distribution | High | ★★★★☆ | ★★☆☆☆ | ★★★★★ | ★★★★★ | ★★★☆☆ |
| LLM SDK ecosystem | High | ★★★★★ | ★★★★☆ | ★★☆☆☆ | ★★☆☆☆ | ★★★★★ |
| MVP development speed | Medium | ★★★★★ | ★★★★☆ | ★★☆☆☆ | ★★★☆☆ | ★★★★☆ |
| Plugin/extension ecosystem | Low | ★★★★★ | ★★★★☆ | ★★☆☆☆ | ★★☆☆☆ | ★★★★★ |
| Binary size & memory | Medium | ★★★☆☆ | ★★☆☆☆ | ★★★★★ | ★★★★☆ | ★★★☆☆ |
| Type safety & correctness | High | ★★★★☆ | ★★☆☆☆ | ★★★★★ | ★★★☆☆ | ★★★★☆ |
| Community & contributors | Low | ★★★★★ | ★★★★★ | ★★★☆☆ | ★★★☆☆ | ★★★★☆ |
| Terminal UI libraries | Medium | ★★★★★ | ★★★★☆ | ★★★☆☆ | ★★★☆☆ | ★★★★★ |

---

## 4. Deep Dive: Testability, Code Organisation, and Maintainability

These three criteria are paramount for a long-lived project. Extensibility is a nice-to-have but not a primary driver. Here's how the top contenders — Rust and Python — compare on these dimensions specifically.

### 4.1 Testability

| Aspect | Rust | Python | TypeScript |
|---|---|---|---|
| **Built-in test runner** | `#[test]` is part of the language. `cargo test` runs unit and integration tests with zero config. Tests live next to the code they test. | `pytest` is excellent but external. Requires setup (`conftest.py`, fixtures, `pytest.ini`). | `vitest`/`jest` are external. Require config files, ts-jest transforms, etc. |
| **Mocking & dependency injection** | Traits make DI natural: `fn new(provider: impl ProviderAdapter)`. Mocking via trait implementations. No magic — everything is explicit. | ABC + duck typing makes DI easy. `unittest.mock` / `pytest-mock` are powerful but rely on runtime patching (monkeypatch), which can hide bugs. | Interfaces + DI work well. Mocking libraries (jest.mock) use runtime magic similar to Python. |
| **Property-based testing** | `proptest` and `quickcheck` crates are mature. | `hypothesis` is best-in-class for property testing. | `fast-check` exists but is less commonly used. |
| **Compile-time bug prevention** | The type system, borrow checker, and exhaustive match eliminate null pointer errors, data races, and unhandled enum variants. Many bugs that need tests in other languages simply cannot compile in Rust. | No compile-time guarantees. mypy covers some cases but is optional, gradual, and incomplete. | TypeScript's type system catches many errors but has escape hatches (`any`, type assertions). |
| **Integration test isolation** | `cargo test` runs tests in parallel by default. Each test binary is isolated. | Tests run in a single process. Shared state between tests is a common source of flaky tests. | Similar to Python — shared process, need discipline for isolation. |

**Verdict:** Rust has the strongest testability story. The type system itself acts as a test suite — if it compiles, entire categories of bugs are ruled out. Python's `pytest` is more ergonomic for writing tests, but Rust needs fewer tests to achieve the same confidence.

### 4.2 Code Organisation

| Aspect | Rust | Python | TypeScript |
|---|---|---|---|
| **Module system** | `mod` + `pub`/`pub(crate)` gives fine-grained visibility control. You can expose a clean public API while keeping internals private. The file system maps 1:1 to the module tree. | Packages and modules work but visibility is convention-based (`_prefix` for private). No enforcement — anyone can import anything. | ES modules with `export`. Visibility is by omission (don't export it). No compiler-enforced encapsulation. |
| **Interface contracts** | `trait` with required methods. The compiler enforces that every implementor satisfies the contract. Associated types and generics enable expressive, type-safe abstractions. | ABC (Abstract Base Class) with `@abstractmethod`. Enforcement is at instantiation time (runtime), not import time. Duck typing means contracts are often implicit. | `interface` declarations. Enforced at compile time by `tsc`. Less expressive than Rust traits (no associated types, no default impls). |
| **Enums / sum types** | `enum` with data variants + exhaustive `match`. Perfect for modelling `OutputEvent`, `PermissionDecision`, `ChatResponseChunk`. Adding a new variant is a compile error everywhere it's not handled. | No native sum types. Approximated with `Union` type hints, `@dataclass`, and manual dispatch. Easy to miss a case. | Discriminated unions with `switch` narrowing. Good but not compiler-enforced exhaustiveness by default (needs `never` check). |
| **Project structure** | Workspace with multiple crates (e.g., `nexus-core`, `nexus-cli`, `nexus-providers`). Clear dependency boundaries enforced by `Cargo.toml`. | Packages with `__init__.py`. Circular imports are a common pain point. No enforced dependency direction. | Monorepo with `packages/` or a single `src/` tree. No built-in dependency boundary enforcement (need tools like NX or turborepo). |

**Verdict:** Rust's module system, traits, and enums are purpose-built for the kind of interface-heavy, multi-component architecture in our design. Every component boundary from the architecture overview maps directly to a Rust crate or module with enforced contracts.

### 4.3 Maintainability

| Aspect | Rust | Python | TypeScript |
|---|---|---|---|
| **Refactoring safety** | Extremely high. Renaming a trait method, changing an enum variant, or modifying a function signature triggers compiler errors at every call site. Refactoring is fearless. | Low. Refactoring relies on IDE support and test coverage. Runtime errors from missed call sites are common. | Moderate. `tsc` catches type-level breakages, but `any` types and dynamic patterns create blind spots. |
| **Long-term readability** | High. Explicit types, no hidden control flow, ownership semantics make data flow obvious. The learning curve is steep but the code reads clearly once understood. | Moderate. Dynamic typing means you often need to trace through code to understand what types are flowing. Type hints help but are optional and sometimes stale. | Moderate-High. Type annotations help readability. But the ecosystem's love of callbacks, promises, and meta-programming (decorators, proxies) can hurt. |
| **Dependency management** | `cargo` is excellent. `Cargo.lock` ensures reproducible builds. `cargo audit` checks for known vulnerabilities. Crate ecosystem is small but high-quality. | `pip` + `requirements.txt` is fragile. `poetry`/`pdm` improve things but the ecosystem is fragmented. Dependency conflicts are a recurring headache. | `npm` is massive but `node_modules` bloat and supply-chain attacks are real risks. `package-lock.json` helps reproducibility. |
| **Error handling** | `Result<T, E>` forces every error to be handled or explicitly propagated (`?`). No silent failures, no uncaught exceptions. | Exceptions can be thrown from anywhere. It's easy to forget a `try/except`. No compiler guidance on what can fail. | Same as Python — `throw`/`catch` is untracked by the type system. |
| **Upgrading and evolving** | The compiler is your safety net. Bumping a dependency version or changing an internal API tells you exactly what broke. | Risky without comprehensive test coverage. Type checkers help but are incomplete. | Moderate. TypeScript compiler catches some issues, but runtime failures from `any`-typed boundaries are possible. |

**Verdict:** Rust is the most maintainable choice for a long-lived project. The initial investment in satisfying the compiler pays dividends in every future refactor, dependency upgrade, and feature addition.

### 4.4 Revised Comparison (Weighted for Your Priorities)

| Criterion | Weight | Rust | Python | TypeScript |
|---|---|---|---|---|
| **Testability** | High | ★★★★★ | ★★★★☆ | ★★★★☆ |
| **Code organisation** | High | ★★★★★ | ★★★☆☆ | ★★★★☆ |
| **Maintainability** | High | ★★★★★ | ★★★☆☆ | ★★★★☆ |
| **Runtime performance** | High | ★★★★★ | ★★☆☆☆ | ★★★☆☆ |
| **LLM SDK ecosystem** | Medium | ★★☆☆☆ | ★★★★☆ | ★★★★★ |
| **MVP development speed** | Medium | ★★☆☆☆ | ★★★★☆ | ★★★★★ |
| **Extensibility** | Low | ★★★☆☆ | ★★★★☆ | ★★★★★ |
| **Distribution** | Medium | ★★★★★ | ★★☆☆☆ | ★★★★☆ |
| **Weighted score** | | **High** | **Moderate** | **Moderate-High** |

---

## 5. Recommendation

**Primary recommendation: Rust (Option C).**

With testability, code organisation, and maintainability as the top priorities, Rust is the strongest choice. Its trait system maps directly to the component interfaces defined in the architecture (ProviderAdapter, Tool, PermissionGate). Its enum system is ideal for the discriminated unions throughout the design (OutputEvent, ChatResponseChunk, PermissionDecision). And its compiler provides a level of refactoring safety that no other option can match.

The main risk is the weak LLM SDK ecosystem. This is mitigated by the fact that LLM provider APIs are HTTP-based and relatively simple — writing thin adapter clients with `reqwest` and `serde` is straightforward, and the provider abstraction layer in the architecture is specifically designed to keep adapters thin. Several community crates (`async-openai`, `anthropic-sdk-rs`) exist and can serve as starting points.

The slower MVP development speed is a real trade-off. Mitigation strategies include starting with a single provider adapter (e.g., Anthropic or OpenAI-compatible, which also covers Ollama), deferring the terminal markdown renderer to a later sprint (plain-text first), and leveraging Rust's excellent crate ecosystem for CLI scaffolding (`clap`), async runtime (`tokio`), HTTP (`reqwest`), and JSON (`serde_json`).

**Fallback recommendation:** If the team determines that Rust's development velocity is too slow for the MVP timeline, TypeScript remains a strong second choice — it scores well on the revised criteria and can be evolved toward the Hybrid (Option E) approach later. Python is not recommended due to its distribution difficulties and weak compile-time guarantees.

---

## 6. Rust-Specific Architecture Mapping

To ground the recommendation, here is how the component architecture maps to Rust:

```
nexus-workspace/                  (Cargo workspace)
├── Cargo.toml                    (workspace definition)
├── crates/
│   ├── nexus-core/               (orchestrator, context manager, permission gate)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── orchestrator.rs   (agentic loop + unit tests at bottom)
│   │       ├── context.rs        (context manager + unit tests)
│   │       ├── permission.rs     (permission gate + unit tests)
│   │       └── types.rs          (Message, OutputEvent, ToolResult, etc.)
│   ├── nexus-providers/          (provider adapters)
│   │   └── src/
│   │       ├── lib.rs            (ProviderAdapter trait)
│   │       ├── anthropic.rs      (adapter + unit tests)
│   │       ├── openai.rs         (adapter + unit tests)
│   │       ├── ollama.rs         (adapter + unit tests)
│   │       └── openai_compat.rs
│   ├── nexus-tools/              (tool implementations)
│   │   └── src/
│   │       ├── lib.rs            (Tool trait, ToolRegistry)
│   │       ├── file_read.rs      (impl + unit tests)
│   │       ├── file_write.rs     (impl + unit tests)
│   │       ├── file_edit.rs      (impl + unit tests)
│   │       ├── shell.rs          (impl + unit tests)
│   │       ├── glob.rs           (impl + unit tests)
│   │       └── grep.rs           (impl + unit tests)
│   ├── nexus-config/             (configuration system)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── loader.rs         (loading + unit tests)
│   │       ├── schema.rs         (validation + unit tests)
│   │       └── merge.rs          (merge logic + unit tests)
│   └── nexus-cli/                (CLI interface — the binary crate)
│       └── src/
│           ├── main.rs
│           ├── repl.rs
│           ├── renderer.rs
│           └── commands.rs
└── tests/                        (integration tests — test crates as a consumer would)
    ├── agentic_loop_test.rs      (end-to-end: prompt → tool calls → response)
    ├── provider_mock_test.rs     (provider adapter contract tests)
    └── tool_permission_test.rs   (permission gate + tool interaction tests)
```

### 6.1 Where Tests Live

Rust has a two-tier test convention:

**Unit tests — inside the source file.** Every `.rs` file can contain a `#[cfg(test)] mod tests` block at the bottom. These tests compile *only* when running `cargo test`, not in release builds. They have access to private functions and internals of the module, making them ideal for testing implementation details.

```rust
// permission.rs

pub fn check(tool: &ToolSchema, args: &Args, policy: &Policy) -> PermissionDecision {
    // ... implementation ...
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list_blocks_rm_rf() {
        let policy = Policy {
            shell_deny_patterns: vec![r"rm\s+-rf\s+/".into()],
            ..Default::default()
        };
        let result = check(&shell_tool(), &args("rm -rf /"), &policy);
        assert!(matches!(result, PermissionDecision::Deny { .. }));
    }

    #[test]
    fn auto_approve_condition_matches() {
        let policy = Policy { /* ... */ };
        let result = check(&shell_tool(), &args("cargo test"), &policy);
        assert!(matches!(result, PermissionDecision::Allow));
    }
}
```

**Integration tests — in the top-level `tests/` directory.** These are separate crates that can only access the *public* API of your library crates. They test cross-component behaviour: "given this prompt and this mock provider response, does the orchestrator produce the right tool calls and feed results back correctly?"

**Why this matters for Nexus:** every component design doc defines interfaces (traits). Unit tests in each file verify that each implementation satisfies its trait contract. Integration tests verify that the components work together through their public APIs. There's no separate `__tests__/` folder to maintain or keep in sync — the tests are always right next to the code.

### 6.2 TDD in Rust

TDD works naturally in Rust and is arguably *more effective* than in dynamically-typed languages because the compiler participates in the red-green-refactor cycle.

**The TDD loop in Rust:**

```
1. RED:    Write a #[test] that describes the behaviour you want.
           → cargo test → it either fails to COMPILE or fails the assertion.

2. GREEN:  Write the minimum code to make it pass.
           → cargo test → all tests pass.

3. REFACTOR: Clean up, extract functions, improve types.
           → cargo test → still passes. Compiler catches any breakage.
```

**What makes Rust TDD distinctive:**

The "Red" phase has two sub-steps that other languages don't. First the test won't compile (the function doesn't exist, or the types don't match), then it compiles but fails the assertion. This gives you an extra feedback signal — the compiler tells you when your interfaces are wrong *before* you even run the test.

**Mocking with traits for DI:**

Every major interface in the Nexus design is a trait. This makes dependency injection and mocking straightforward:

```rust
// In production code: the orchestrator accepts any ProviderAdapter
struct Orchestrator<P: ProviderAdapter> {
    provider: P,
    // ...
}

// In tests: a mock that returns canned responses
struct MockProvider {
    responses: Vec<ChatResponse>,
}

impl ProviderAdapter for MockProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        Ok(self.responses.pop().unwrap())
    }
    // ...
}

#[test]
fn orchestrator_feeds_tool_result_back_to_provider() {
    let mock = MockProvider {
        responses: vec![
            response_with_tool_call("file_read", json!({"path": "src/main.rs"})),
            response_with_text("Here's what I found..."),
        ],
    };
    let orchestrator = Orchestrator::new(mock, /* ... */);
    let events: Vec<_> = block_on(orchestrator.run("read main.rs")).collect();
    
    assert!(events.iter().any(|e| matches!(e, OutputEvent::ToolCall { .. })));
    assert!(events.iter().any(|e| matches!(e, OutputEvent::Done)));
}
```

For more sophisticated mocking, the `mockall` crate auto-generates mock implementations from trait definitions:

```rust
#[automock]
trait ProviderAdapter {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    async fn validate(&self) -> Result<()>;
    fn context_window_size(&self) -> usize;
}

#[test]
fn validates_provider_on_startup() {
    let mut mock = MockProviderAdapter::new();
    mock.expect_validate()
        .times(1)
        .returning(|| Ok(()));
    mock.expect_context_window_size()
        .returning(|| 200_000);

    let config = OrchestratorConfig { provider: mock, /* ... */ };
    // ... assert startup calls validate exactly once
}
```

**Useful testing crates for Nexus:**

| Crate | Purpose |
|---|---|
| `mockall` | Auto-generate mock implementations from traits |
| `assert_cmd` | Test CLI binaries (run `nexus` as a subprocess, check stdout/stderr/exit code) |
| `tempfile` | Create temp directories for file tool tests (auto-cleaned up) |
| `wiremock` | Mock HTTP servers for provider adapter tests (simulate Anthropic/OpenAI APIs) |
| `proptest` | Property-based testing (e.g., "for any valid config YAML, merge never panics") |
| `tokio::test` | `#[tokio::test]` macro for async test functions |
| `insta` | Snapshot testing — great for verifying rendered output and serialised config |

### 6.3 Key Rust Patterns

- **Traits:** `ProviderAdapter`, `Tool`, `PermissionGate`, `OutputRenderer`, `InputSource`, `SessionLogger` — all component interfaces become traits. Every trait is directly mockable for testing.
- **Enums:** `OutputEvent`, `ChatResponseChunk`, `PermissionDecision`, `ProviderError` — all discriminated unions become Rust enums with exhaustive matching. Adding a variant is a compile error everywhere it's not handled.
- **`async_trait`:** Provider adapters and tools use async methods. The `async-trait` crate bridges Rust's current async-in-trait limitations.
- **`serde`:** All configuration types derive `Serialize` / `Deserialize` for YAML loading. Tool schemas use `serde_json::Value` for JSON Schema representation.
- **Feature flags:** Optional provider adapters can be behind Cargo feature flags (e.g., `features = ["anthropic", "openai", "ollama"]`).

---

## 7. Open Questions

1. **Bun vs. Node?** Bun offers faster startup (~50ms vs. ~200ms) and built-in bundling, but its ecosystem compatibility is still maturing. Worth evaluating as the Node runtime alternative.
2. **Deno?** Deno offers built-in TypeScript support and better security defaults, but its npm compatibility layer adds friction and some packages don't work.
3. **WASM plugins?** If the agent supports user-defined tools via WASM, any language can produce plugins. This decouples the plugin language from the core language but adds runtime complexity.

---

*This decision should be revisited after the MVP ships and real-world performance data is available.*
