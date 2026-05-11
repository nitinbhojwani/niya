//! Configuration validation.
//!
//! Validates a resolved config for required fields and consistency.

use crate::ResolvedConfig;

/// Validation error.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("No provider configured. Run `nexus --setup` to get started.")]
    NoProvider,

    #[error("Default provider '{0}' is not configured in providers list.")]
    InvalidDefaultProvider(String),

    #[error("Provider '{provider}' is missing required field: {field}")]
    MissingField { provider: String, field: String },
}

/// Validate the resolved configuration.
pub fn validate(config: &ResolvedConfig) -> Result<(), Vec<ConfigError>> {
    let mut errors = Vec::new();

    // Must have at least one provider
    if config.providers.is_empty() {
        errors.push(ConfigError::NoProvider);
        return Err(errors);
    }

    // Default provider must exist in the providers map
    if !config.default_provider.is_empty()
        && !config.providers.contains_key(&config.default_provider)
    {
        errors.push(ConfigError::InvalidDefaultProvider(
            config.default_provider.clone(),
        ));
    }

    // Each provider must have a default_model
    for (name, provider) in &config.providers {
        if provider.default_model.is_empty() {
            errors.push(ConfigError::MissingField {
                provider: name.clone(),
                field: "default_model".to_string(),
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderConfig;

    #[test]
    fn validate_rejects_empty_config() {
        let config = ResolvedConfig::default();
        let result = validate(&config);
        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let mut config = ResolvedConfig::default();
        config.default_provider = "openai".to_string();
        config.providers.insert(
            "openai".to_string(),
            ProviderConfig {
                api_key: Some("sk-test".to_string()),
                base_url: None,
                default_model: "gpt-4o".to_string(),
                max_retries: 3,
            },
        );

        let result = validate(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_invalid_default_provider() {
        let mut config = ResolvedConfig::default();
        config.default_provider = "nonexistent".to_string();
        config.providers.insert(
            "openai".to_string(),
            ProviderConfig {
                api_key: Some("sk-test".to_string()),
                base_url: None,
                default_model: "gpt-4o".to_string(),
                max_retries: 3,
            },
        );

        let result = validate(&config);
        assert!(result.is_err());
    }
}
