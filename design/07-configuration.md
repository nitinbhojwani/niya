# Niya Component Design: Configuration System

**Covers Requirements:** INST-01 through INST-03, PROV-03, PROV-04, PERM-03  
**Status:** Draft  

---

## 1. Purpose

The Configuration System loads, merges, validates, and provides typed access to all agent settings. It supports a layered configuration model where settings can be defined at multiple levels (global, project, CLI flags, environment variables), with a clear precedence order.

---

## 2. Responsibilities

- Load configuration from multiple sources and merge them by precedence.
- Validate configuration against a known schema and surface clear errors.
- Resolve environment variable references in config values (e.g., `${ANTHROPIC_API_KEY}`).
- Provide typed access to configuration values for all other components.
- Support first-run interactive setup for initial provider configuration.

---

## 3. Interfaces

### 3.1 ConfigManager

```
interface ConfigManager {
  // Load and merge configuration from all sources.
  // CLI args override project config, which overrides global config, which overrides defaults.
  load(cliArgs: CLIArgs): ResolvedConfig

  // Check if a first-run setup is needed (no config file exists, no provider configured).
  needsSetup(): boolean

  // Run interactive first-run setup.
  runSetup(): Promise<void>

  // Get the path to the active global config file.
  globalConfigPath(): string

  // Get the path to the active project config file (or null if none).
  projectConfigPath(): string | null
}
```

### 3.2 ResolvedConfig (fully merged and validated)

```
interface ResolvedConfig {
  // Provider settings
  providers:       Record<string, ProviderConfig>
  defaultProvider: string
  defaultModel:    string

  // Permission settings
  permissions:     PermissionPolicy

  // Context settings
  context: {
    maxProjectContextLines: number      // default: 200
    projectInstructionFile: string      // default: "NIYA.md"
    respectGitignore:       boolean     // default: true
  }

  // Session settings
  session: {
    maxIterations:      number          // default: 20
    logDirectory:       string          // default: ".niya/sessions"
    shellTimeout:       number          // default: 30000 (ms)
    shellOutputLimit:   number          // default: 100000 (chars)
  }

  // Display settings
  display: {
    color:              boolean         // default: true
    markdown:           boolean         // default: true
    verbose:            boolean         // default: false
  }

  // Resolved project root
  projectRoot: string
}

interface ProviderConfig {
  apiKey?:       string                // resolved from env var or plaintext
  baseUrl?:      string                // for custom endpoints
  defaultModel:  string
  maxRetries:    number                // default: 3
}
```

---

## 4. Configuration Sources and Precedence

```
Priority (highest → lowest):

1. CLI flags           --provider anthropic --model claude-sonnet-4-6 --no-color
2. Environment vars    AGENT_DEFAULT_PROVIDER=anthropic
3. Project config      <projectRoot>/.niya/config.yaml
4. Global config       ~/.niya/config.yaml
5. Built-in defaults   (hardcoded sensible defaults)
```

### Merge rules:
- Scalar values: higher-precedence wins.
- Objects (e.g., `providers`): deep-merged. Higher-precedence keys override matching lower-precedence keys; non-conflicting keys are preserved.
- Arrays (e.g., `shell_deny_patterns`): higher-precedence replaces the entire array (no merge), unless the array uses an explicit `+append` syntax.

---

## 5. Config File Format

YAML is the primary format. The schema is the same for both global and project-level files.

```yaml
# ~/.niya/config.yaml (global)
providers:
  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"
    default_model: "claude-sonnet-4-6"
  openai:
    api_key: "${OPENAI_API_KEY}"
    default_model: "gpt-4o"
  ollama:
    base_url: "http://localhost:11434"
    default_model: "llama3"

default_provider: "anthropic"

permissions:
  default: "ask"
  tools:
    file_read:
      level: "auto"
    grep:
      level: "auto"
    glob:
      level: "auto"
  shell_deny_patterns:
    - "rm\\s+-rf\\s+/"
    - "mkfs"

session:
  max_iterations: 25
  shell_timeout: 60000

display:
  color: true
  markdown: true
```

```yaml
# <projectRoot>/.niya/config.yaml (project-level override)
default_provider: "ollama"             # this project uses local models

permissions:
  tools:
    shell_execute:
      level: "auto"
      auto_approve_when:
        - arg: "command"
          matches: "^(npm test|npm run build)$"

context:
  project_instruction_file: "CONTRIBUTING.md"
```

---

## 6. Environment Variable Resolution

Any config value can reference an environment variable using `${VAR_NAME}` syntax:

```
function resolveEnvVars(value):
  if value is string:
    return value.replaceAll(/\$\{(\w+)\}/g, (match, varName) => {
      envValue = process.env[varName]
      if envValue is undefined:
        warn("Environment variable {varName} is not set")
        return ""
      return envValue
    })
  return value
```

This is applied recursively to all string values in the config after loading.

---

## 7. Config Validation

After merging and env-var resolution, the config is validated against a JSON Schema. Validation errors are reported with the source file and path:

```
Error: Invalid configuration
  → providers.anthropic.api_key: must be a non-empty string
    Source: ~/.niya/config.yaml, line 3
  → session.max_iterations: must be a positive integer, got "abc"
    Source: .niya/config.yaml, line 8
```

---

## 8. Project Root Detection

The project root is detected by walking up from the current working directory until one of these markers is found:

```
Priority order:
1. .niya/          (niya-specific config directory)
2. .git/            (git repository root)
3. package.json     (Node.js project)
4. Cargo.toml       (Rust project)
5. pyproject.toml   (Python project)
6. go.mod           (Go project)
7. Makefile         (generic project)
```

If no marker is found, the current working directory is used as the project root with a warning. The user can override with `--project-root`.

---

## 9. First-Run Setup

When `needsSetup()` returns true (no global config file exists), the agent runs an interactive setup:

```
Welcome to Niya! Let's get you set up.

Which LLM provider would you like to use?
  1. Anthropic (Claude)
  2. OpenAI (GPT)
  3. Ollama (local)
  4. Custom (OpenAI-compatible endpoint)

> 1

Enter your Anthropic API key (or press Enter to use ANTHROPIC_API_KEY env var):
> sk-ant-...

Which model? (default: claude-sonnet-4-6)
> 

✓ Configuration saved to ~/.niya/config.yaml
✓ Connection verified — you're ready to go!

Type 'niya' to start a session.
```

---

## 10. Secure Key Storage

API keys in the config file can be stored in three ways:

1. **Environment variable reference** (recommended): `api_key: "${ANTHROPIC_API_KEY}"` — the key never touches disk.
2. **Plaintext in config** (acceptable for local-only use): `api_key: "sk-ant-..."` — the config file should have `600` permissions.
3. **OS keychain** (Phase 2): `api_key: "keychain:anthropic-api-key"` — stored in the OS credential manager.

In MVP, options 1 and 2 are supported. The setup wizard recommends option 1.

---

## 11. Error Handling

| Case | Behaviour |
|---|---|
| Config file has syntax errors (invalid YAML) | Print the parse error with file path and line number. Exit with code 1. |
| Required field missing (e.g., no provider configured) | Print which field is missing and suggest running `niya --setup`. Exit with code 1. |
| Environment variable not set | Warn at load time. If the variable is required (e.g., API key), error at provider validation. |
| Both global and project config have conflicting values | Project config wins. No warning (this is expected behaviour). |

---

## 12. Future Phase Notes

- **Phase 2:** OS keychain integration for secure key storage. Config supports hook definitions and MCP server declarations. A `niya config` sub-command provides get/set access.
- **Phase 3:** Team config files are loaded from a remote source (e.g., a Git repo) and merged with the same precedence model. Org-level overrides are marked as non-overridable by individuals.
