//! Niya configuration system.
//!
//! Loads, merges, validates, and provides typed access to all settings.
//! Supports layered configuration: CLI flags > env vars > project config >
//! global config > defaults.

pub mod loader;
pub mod merge;
pub mod schema;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Resolved configuration (fully merged)
// ---------------------------------------------------------------------------

/// Fully resolved configuration after merging all sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfig {
    pub providers: HashMap<String, ProviderConfig>,
    pub default_provider: String,

    #[serde(default)]
    pub permissions: PermissionPolicy,

    #[serde(default)]
    pub context: ContextConfig,

    #[serde(default)]
    pub session: SessionConfig,

    #[serde(default)]
    pub display: DisplayConfig,

    #[serde(skip)]
    pub project_root: PathBuf,
}

/// Configuration for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    pub default_model: String,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

/// Permission policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    #[serde(default = "default_permission_level")]
    pub default_level: String,
    #[serde(default)]
    pub tools: HashMap<String, ToolPermission>,
    #[serde(default)]
    pub shell_deny_patterns: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
}

/// Per-tool permission configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    pub level: String,
    #[serde(default)]
    pub auto_approve_when: Vec<ArgCondition>,
}

/// A condition for auto-approving a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgCondition {
    pub arg: String,
    pub matches: String,
}

/// Context-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    #[serde(default = "default_max_context_lines")]
    pub max_project_context_lines: usize,
    #[serde(default = "default_instruction_file")]
    pub project_instruction_file: String,
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,
}

/// Session-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    #[serde(default = "default_log_directory")]
    pub log_directory: String,
    #[serde(default = "default_shell_timeout")]
    pub shell_timeout: u64,
    #[serde(default = "default_shell_output_limit")]
    pub shell_output_limit: usize,
}

/// Display-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_true")]
    pub color: bool,
    #[serde(default = "default_true")]
    pub markdown: bool,
    #[serde(default)]
    pub verbose: bool,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

fn default_max_retries() -> u32 {
    3
}
fn default_permission_level() -> String {
    "ask".to_string()
}
fn default_max_context_lines() -> usize {
    200
}
fn default_instruction_file() -> String {
    "NIYA.md".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_iterations() -> usize {
    20
}
fn default_log_directory() -> String {
    ".niya/sessions".to_string()
}
fn default_shell_timeout() -> u64 {
    30_000
}
fn default_shell_output_limit() -> usize {
    100_000
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            default_level: default_permission_level(),
            tools: HashMap::new(),
            shell_deny_patterns: vec![
                r"rm\s+-rf\s+/".to_string(),
                r"mkfs".to_string(),
                r"dd\s+if=".to_string(),
                r":()\{ :|:& \};:".to_string(),
            ],
            allowed_paths: vec![],
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_project_context_lines: default_max_context_lines(),
            project_instruction_file: default_instruction_file(),
            respect_gitignore: true,
        }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            log_directory: default_log_directory(),
            shell_timeout: default_shell_timeout(),
            shell_output_limit: default_shell_output_limit(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            color: true,
            markdown: true,
            verbose: false,
        }
    }
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: String::new(),
            permissions: PermissionPolicy::default(),
            context: ContextConfig::default(),
            session: SessionConfig::default(),
            display: DisplayConfig::default(),
            project_root: PathBuf::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let config = ResolvedConfig::default();
        assert_eq!(config.session.max_iterations, 20);
        assert_eq!(config.session.shell_timeout, 30_000);
        assert_eq!(config.permissions.default_level, "ask");
        assert!(config.display.color);
        assert!(config.display.markdown);
    }

    #[test]
    fn permission_policy_default_has_deny_patterns() {
        let policy = PermissionPolicy::default();
        assert!(!policy.shell_deny_patterns.is_empty());
    }
}
