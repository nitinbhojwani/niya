//! Permission gate.
//!
//! Gates every tool invocation by evaluating the configured permission policy.
//! Returns Allow, Deny, or Ask decisions. Shell deny-list patterns and path
//! boundary checks are enforced here.

use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use crate::types::{PermissionDecision, ToolSchema};
use nexus_config::PermissionPolicy;

/// The permission gate that checks every tool invocation.
pub struct PermissionGate {
    policy: PermissionPolicy,
    project_root: std::path::PathBuf,
    /// Tools auto-approved for this session (via "always" response).
    session_overrides: Mutex<HashSet<String>>,
}

impl PermissionGate {
    pub fn new(policy: PermissionPolicy, project_root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            policy,
            project_root: project_root.into(),
            session_overrides: Mutex::new(HashSet::new()),
        }
    }

    /// Check whether a tool call is permitted.
    pub fn check(
        &self,
        tool: &ToolSchema,
        args: &serde_json::Value,
    ) -> PermissionDecision {
        // Check session overrides first
        {
            let overrides = self.session_overrides.lock().unwrap();
            if overrides.contains(&tool.name) {
                return PermissionDecision::Allow;
            }
        }

        // Check shell deny-list
        if tool.name == "shell_execute" {
            if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                for pattern in &self.policy.shell_deny_patterns {
                    if let Ok(re) = Regex::new(pattern) {
                        if re.is_match(cmd) {
                            return PermissionDecision::Deny {
                                reason: format!("Command matches deny pattern: {}", pattern),
                            };
                        }
                    }
                }
            }
        }

        // Check path boundaries for file tools
        if matches!(
            tool.name.as_str(),
            "file_read" | "file_write" | "file_edit"
        ) {
            if let Some(path_str) = args.get("file_path").and_then(|v| v.as_str()) {
                let resolved = self.project_root.join(path_str);
                if let Ok(canonical) = resolved.canonicalize() {
                    if !canonical.starts_with(&self.project_root) {
                        return PermissionDecision::Deny {
                            reason: "Path is outside the project root".to_string(),
                        };
                    }
                }
            }
        }

        // Look up tool-specific permission level
        let level = self
            .policy
            .tools
            .get(&tool.name)
            .map(|tp| tp.level.as_str())
            .unwrap_or(self.policy.default_level.as_str());

        match level {
            "deny" => PermissionDecision::Deny {
                reason: "Tool is disabled by policy".to_string(),
            },
            "auto" => PermissionDecision::Allow,
            "ask" | _ => {
                // Check auto-approve conditions
                if let Some(tool_perm) = self.policy.tools.get(&tool.name) {
                    for condition in &tool_perm.auto_approve_when {
                        if let Some(arg_val) = args.get(&condition.arg).and_then(|v| v.as_str()) {
                            if let Ok(re) = Regex::new(&condition.matches) {
                                if re.is_match(arg_val) {
                                    return PermissionDecision::Allow;
                                }
                            }
                        }
                    }
                }

                PermissionDecision::Ask {
                    message: format!(
                        "Allow {} with args {}?",
                        tool.name,
                        serde_json::to_string_pretty(args).unwrap_or_default()
                    ),
                }
            }
        }
    }

    /// Mark a tool as auto-approved for the rest of this session.
    pub fn session_approve(&self, tool_name: &str) {
        let mut overrides = self.session_overrides.lock().unwrap();
        overrides.insert(tool_name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_config::{ArgCondition, ToolPermission};
    use std::collections::HashMap;

    fn test_policy() -> PermissionPolicy {
        let mut tools = HashMap::new();
        tools.insert(
            "file_read".to_string(),
            ToolPermission {
                level: "auto".to_string(),
                auto_approve_when: vec![],
            },
        );
        tools.insert(
            "shell_execute".to_string(),
            ToolPermission {
                level: "ask".to_string(),
                auto_approve_when: vec![ArgCondition {
                    arg: "command".to_string(),
                    matches: "^cargo test$".to_string(),
                }],
            },
        );

        PermissionPolicy {
            default_level: "ask".to_string(),
            tools,
            shell_deny_patterns: vec![r"rm\s+-rf\s+/".to_string()],
            allowed_paths: vec![],
        }
    }

    fn tool_schema(name: &str) -> ToolSchema {
        ToolSchema {
            name: name.to_string(),
            description: String::new(),
            parameters: serde_json::json!({}),
        }
    }

    #[test]
    fn auto_level_allows_immediately() {
        let gate = PermissionGate::new(test_policy(), "/tmp/project");
        let decision = gate.check(
            &tool_schema("file_read"),
            &serde_json::json!({"file_path": "src/main.rs"}),
        );
        assert!(matches!(decision, PermissionDecision::Allow));
    }

    #[test]
    fn deny_list_blocks_dangerous_commands() {
        let gate = PermissionGate::new(test_policy(), "/tmp/project");
        let decision = gate.check(
            &tool_schema("shell_execute"),
            &serde_json::json!({"command": "rm -rf /"}),
        );
        assert!(matches!(decision, PermissionDecision::Deny { .. }));
    }

    #[test]
    fn auto_approve_condition_matches() {
        let gate = PermissionGate::new(test_policy(), "/tmp/project");
        let decision = gate.check(
            &tool_schema("shell_execute"),
            &serde_json::json!({"command": "cargo test"}),
        );
        assert!(matches!(decision, PermissionDecision::Allow));
    }

    #[test]
    fn ask_level_prompts_for_unknown_commands() {
        let gate = PermissionGate::new(test_policy(), "/tmp/project");
        let decision = gate.check(
            &tool_schema("shell_execute"),
            &serde_json::json!({"command": "npm install express"}),
        );
        assert!(matches!(decision, PermissionDecision::Ask { .. }));
    }

    #[test]
    fn session_override_upgrades_to_auto() {
        let gate = PermissionGate::new(test_policy(), "/tmp/project");
        // First check should ask
        let decision = gate.check(
            &tool_schema("shell_execute"),
            &serde_json::json!({"command": "npm install express"}),
        );
        assert!(matches!(decision, PermissionDecision::Ask { .. }));

        // After session approval, should allow
        gate.session_approve("shell_execute");
        let decision = gate.check(
            &tool_schema("shell_execute"),
            &serde_json::json!({"command": "npm install express"}),
        );
        assert!(matches!(decision, PermissionDecision::Allow));
    }
}
