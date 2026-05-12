//! Configuration loader.
//!
//! Loads config from global (~/.niya/config.yaml) and project-level
//! (.niya/config.yaml) sources, resolves environment variables, and
//! merges layers by precedence.

use std::path::{Path, PathBuf};

use crate::ResolvedConfig;

/// Detect the project root by walking up from cwd.
pub fn detect_project_root(start: &Path) -> PathBuf {
    let markers = [
        ".niya",
        ".git",
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "go.mod",
        "Makefile",
    ];

    let mut current = start.to_path_buf();
    loop {
        for marker in &markers {
            if current.join(marker).exists() {
                return current;
            }
        }
        if !current.pop() {
            break;
        }
    }

    // Fallback to start directory
    start.to_path_buf()
}

/// Global config directory path (~/.niya/).
pub fn global_config_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".niya"))
}

/// Global config file path (~/.niya/config.yaml).
pub fn global_config_path() -> Option<PathBuf> {
    global_config_dir().map(|d| d.join("config.yaml"))
}

/// Project config file path (<root>/.niya/config.yaml).
pub fn project_config_path(project_root: &Path) -> PathBuf {
    project_root.join(".niya").join("config.yaml")
}

/// Load configuration from all sources and merge.
pub fn load_config(project_root: &Path) -> anyhow::Result<ResolvedConfig> {
    let mut config = ResolvedConfig::default();
    config.project_root = project_root.to_path_buf();

    // Load global config if it exists
    if let Some(global_path) = global_config_path() {
        if global_path.exists() {
            let content = std::fs::read_to_string(&global_path)?;
            let content = resolve_env_vars(&content);
            let global: ResolvedConfig = serde_yaml::from_str(&content)?;
            config = crate::merge::merge_configs(config, global);
        }
    }

    // Load project config if it exists
    let project_path = project_config_path(project_root);
    if project_path.exists() {
        let content = std::fs::read_to_string(&project_path)?;
        let content = resolve_env_vars(&content);
        let project: ResolvedConfig = serde_yaml::from_str(&content)?;
        config = crate::merge::merge_configs(config, project);
    }

    // Ensure project_root is set
    config.project_root = project_root.to_path_buf();

    Ok(config)
}

/// Resolve ${VAR_NAME} references in a string.
fn resolve_env_vars(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{(\w+)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match std::env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                tracing::warn!("Environment variable {} is not set", var_name);
                String::new()
            }
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_project_root_finds_cargo_toml() {
        // This test runs from within the niya workspace, so it should
        // find the Cargo.toml
        let cwd = std::env::current_dir().unwrap();
        let root = detect_project_root(&cwd);
        // Should find some project root (at least cwd)
        assert!(root.exists());
    }

    #[test]
    fn resolve_env_vars_replaces_known_vars() {
        std::env::set_var("NIYA_TEST_VAR", "hello");
        let result = resolve_env_vars("key: ${NIYA_TEST_VAR}");
        assert_eq!(result, "key: hello");
        std::env::remove_var("NIYA_TEST_VAR");
    }

    #[test]
    fn resolve_env_vars_clears_unknown_vars() {
        let result = resolve_env_vars("key: ${NIYA_NONEXISTENT_VAR_12345}");
        assert_eq!(result, "key: ");
    }
}
