# Niya Component Design: Permission Model

**Covers Requirements:** PERM-01 through PERM-05, TOOL-07  
**Status:** Draft  

---

## 1. Purpose

The Permission Model gates every tool invocation, ensuring the user retains control over what the agent does. It evaluates a configured policy for each tool call, prompts the user when needed, blocks denied actions, and logs every decision.

---

## 2. Responsibilities

- Evaluate each tool call against the permission policy before execution.
- Prompt the user for approval when the policy requires it.
- Block tool calls that match deny rules (destructive commands, out-of-bounds paths).
- Log every permission decision (allowed, denied, user-approved, user-declined).

---

## 3. Interfaces

### 3.1 PermissionGate

```
interface PermissionGate {
  // Check whether a tool call is permitted.
  // Returns the decision synchronously if the policy is "auto" or "deny".
  // Returns "ask" if the user must be prompted (handled by the orchestrator).
  check(tool: ToolSchema, args: Record<string, any>): PermissionDecision
}

type PermissionDecision = 
  | { action: "allow" }
  | { action: "deny";  reason: string }
  | { action: "ask";   message: string }
```

### 3.2 PermissionPolicy

The policy is loaded from configuration and defines per-tool permission levels plus global rules.

```
interface PermissionPolicy {
  // Default permission level for tools not explicitly listed.
  defaultLevel: "ask" | "auto" | "deny"

  // Per-tool overrides.
  tools: Record<string, ToolPermission>

  // Deny rules for shell commands.
  shellDenyPatterns: string[]

  // Allowed paths (if set, only these paths are accessible).
  // Default: [projectRoot]
  allowedPaths: string[]
}

interface ToolPermission {
  level: "ask" | "auto" | "deny"
  // Optional: auto-approve only when args match these conditions.
  autoApproveWhen?: ArgCondition[]
}

interface ArgCondition {
  arg:     string         // parameter name
  matches: string         // regex pattern the value must match
}
```

---

## 4. Configuration

```yaml
# .niya/config.yaml (project-level)
permissions:
  default: "ask"

  tools:
    file_read:
      level: "auto"                          # reading is always safe
    file_write:
      level: "ask"
    file_edit:
      level: "ask"
    shell_execute:
      level: "ask"
      auto_approve_when:
        - arg: "command"
          matches: "^(npm test|npm run lint|cargo test|make test)$"  # safe commands
    glob:
      level: "auto"
    grep:
      level: "auto"

  shell_deny_patterns:
    - "rm\\s+-rf\\s+/"
    - "mkfs"
    - "dd\\s+if="
    - ":(){ :|:& };:"          # fork bomb
    - "chmod\\s+-R\\s+777"
    - "curl.*\\|\\s*sh"        # pipe to shell

  allowed_paths:
    - "${PROJECT_ROOT}"
```

---

## 5. Evaluation Logic

```
function check(tool, args):
  // 1. Check global deny rules first
  if tool.name == "shell_execute":
    for pattern in policy.shellDenyPatterns:
      if args.command matches pattern:
        return { action: "deny", reason: "Command matches deny pattern: {pattern}" }

  if tool.name in ["file_read", "file_write", "file_edit"]:
    resolvedPath = resolve(projectRoot, args.file_path)
    if not isWithinAllowedPaths(resolvedPath):
      return { action: "deny", reason: "Path outside allowed directories" }

  // 2. Look up tool-specific permission level
  toolPermission = policy.tools[tool.name] ?? { level: policy.defaultLevel }

  if toolPermission.level == "deny":
    return { action: "deny", reason: "Tool is disabled by policy" }

  if toolPermission.level == "auto":
    return { action: "allow" }

  if toolPermission.level == "ask":
    // Check auto-approve conditions
    if toolPermission.autoApproveWhen:
      for condition in toolPermission.autoApproveWhen:
        if args[condition.arg] matches condition.matches:
          return { action: "allow" }  // auto-approved by condition

    // No condition matched — ask the user
    return { action: "ask", message: formatApprovalMessage(tool, args) }
```

---

## 6. Approval Prompt Format

When the permission gate returns `ask`, the orchestrator delegates to the CLI Interface to display an approval prompt:

```
┌─ shell_execute ───────────────────────────────────
│  Command: npm install express
│  Working directory: /home/user/my-project
│
│  Allow? [y/n/a] (y=yes, n=no, a=always for this tool)
└───────────────────────────────────────────────────
```

The user's response:
- **y (yes):** Allow this single invocation.
- **n (no):** Deny this invocation. Return an error to the model.
- **a (always):** Upgrade this tool's permission to `auto` for the rest of the session (not persisted to config).

---

## 7. Session-Level Overrides

The "always" approval (`a`) creates a session-level override that is stored in memory and takes precedence over the config-level policy. It is never written back to the config file. When the session ends, it is discarded.

```
interface SessionOverrides {
  // Tools whose permission has been upgraded to "auto" for this session.
  autoApprovedTools: Set<string>
}
```

---

## 8. Logging

Every permission decision is logged via the Session Logger:

```json
{
  "event": "permission_check",
  "timestamp": "2026-05-11T14:22:03Z",
  "tool": "shell_execute",
  "args": { "command": "npm test" },
  "decision": "allow",
  "reason": "auto_approve_condition_matched",
  "condition": "^(npm test|npm run lint)$"
}
```

---

## 9. Error Handling

| Case | Behaviour |
|---|---|
| User declines a tool call | Return `ToolResult(success=false, output="User declined this action")`. The model typically proposes an alternative. |
| Deny-list match | Return `ToolResult(success=false, output="Blocked: {reason}")`. Log the attempt. |
| Path outside project root | Return `ToolResult(success=false, output="Access denied: path is outside the project")`. |

---

## 10. Future Phase Notes

- **Phase 2:** Hooks can observe and modify permission decisions (e.g., a corporate hook that blocks `curl` commands). MCP tools go through the same permission gate.
- **Phase 3:** Org-level policies (RBAC) are loaded alongside user policies. The gate evaluates both, with org policies taking precedence. Deny decisions are reported to an admin audit log.
