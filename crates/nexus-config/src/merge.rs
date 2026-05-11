//! Configuration merging.
//!
//! Merges two config layers. The `override_config` takes precedence
//! over the `base` for any fields that are set.

use crate::ResolvedConfig;

/// Merge two configs. `override_cfg` values take precedence over `base`.
pub fn merge_configs(base: ResolvedConfig, override_cfg: ResolvedConfig) -> ResolvedConfig {
    let mut merged = base;

    // Merge providers (deep merge: override individual providers)
    for (name, provider) in override_cfg.providers {
        merged.providers.insert(name, provider);
    }

    // Override default provider if set
    if !override_cfg.default_provider.is_empty() {
        merged.default_provider = override_cfg.default_provider;
    }

    // Merge permissions
    if override_cfg.permissions.default_level != "ask"
        || !override_cfg.permissions.tools.is_empty()
    {
        // Override default level
        merged.permissions.default_level = override_cfg.permissions.default_level;

        // Merge per-tool permissions
        for (name, perm) in override_cfg.permissions.tools {
            merged.permissions.tools.insert(name, perm);
        }

        // Override deny patterns if provided
        if !override_cfg.permissions.shell_deny_patterns.is_empty() {
            merged.permissions.shell_deny_patterns =
                override_cfg.permissions.shell_deny_patterns;
        }
    }

    // Override session config if non-default values are present
    if override_cfg.session.max_iterations != 20 {
        merged.session.max_iterations = override_cfg.session.max_iterations;
    }
    if override_cfg.session.shell_timeout != 30_000 {
        merged.session.shell_timeout = override_cfg.session.shell_timeout;
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderConfig;

    #[test]
    fn merge_overrides_default_provider() {
        let base = ResolvedConfig {
            default_provider: "anthropic".to_string(),
            ..Default::default()
        };
        let override_cfg = ResolvedConfig {
            default_provider: "ollama".to_string(),
            ..Default::default()
        };

        let merged = merge_configs(base, override_cfg);
        assert_eq!(merged.default_provider, "ollama");
    }

    #[test]
    fn merge_deep_merges_providers() {
        let mut base = ResolvedConfig::default();
        base.providers.insert(
            "anthropic".to_string(),
            ProviderConfig {
                api_key: Some("sk-base".to_string()),
                base_url: None,
                default_model: "claude-sonnet-4-6".to_string(),
                max_retries: 3,
            },
        );

        let mut override_cfg = ResolvedConfig::default();
        override_cfg.providers.insert(
            "ollama".to_string(),
            ProviderConfig {
                api_key: None,
                base_url: Some("http://localhost:11434".to_string()),
                default_model: "llama3".to_string(),
                max_retries: 3,
            },
        );

        let merged = merge_configs(base, override_cfg);
        assert!(merged.providers.contains_key("anthropic"));
        assert!(merged.providers.contains_key("ollama"));
    }
}
