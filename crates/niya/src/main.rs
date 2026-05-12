//! Niya CLI — the entry point for the interactive coding agent.
//!
//! Parses command-line arguments with `clap`, loads configuration, initialises
//! the orchestrator, and enters the REPL loop.

mod commands;
mod renderer;
mod repl;

use anyhow::Result;
use clap::Parser;
use niya_core::ProviderAdapter;
use std::path::PathBuf;

/// Niya — an AI coding assistant for your terminal.
#[derive(Parser, Debug)]
#[command(name = "niya", version, about)]
struct Cli {
    /// Initial prompt to send (non-interactive mode if provided with --print).
    #[arg(short, long)]
    prompt: Option<String>,

    /// Provider to use (overrides config).
    #[arg(long)]
    provider: Option<String>,

    /// Model to use (overrides config).
    #[arg(long)]
    model: Option<String>,

    /// Project root directory (default: auto-detect).
    #[arg(long)]
    project: Option<PathBuf>,

    /// Print response and exit (non-interactive mode).
    #[arg(long)]
    print: bool,

    /// Dry-run mode — show tool calls without executing them.
    #[arg(long)]
    dry_run: bool,

    /// Enable verbose output.
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialise tracing
    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with_target(false)
        .init();

    // Load configuration
    let project_root = match &cli.project {
        Some(p) => p.clone(),
        None => {
            let cwd = std::env::current_dir()?;
            niya_config::loader::detect_project_root(&cwd)
        }
    };

    let mut config = niya_config::loader::load_config(&project_root)?;
    config.project_root = project_root.clone();

    // Apply CLI overrides
    if let Some(ref provider) = cli.provider {
        config.default_provider = provider.clone();
    }
    if cli.verbose {
        config.display.verbose = true;
    }

    // Validate configuration
    niya_config::schema::validate(&config).map_err(|errors| {
        anyhow::anyhow!(
            "Configuration errors:\n{}",
            errors
                .iter()
                .map(|e| format!("  - {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })?;

    // Build the provider adapter
    let provider_name = &config.default_provider;
    let provider_config = config
        .providers
        .get(provider_name)
        .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found in configuration", provider_name))?;

    let model = cli
        .model
        .as_deref()
        .unwrap_or(&provider_config.default_model);

    let adapter = niya_providers::OpenAiCompatAdapter::new(
        provider_name,
        provider_config
            .base_url
            .as_deref()
            .unwrap_or("https://api.openai.com/v1"),
        provider_config.api_key.clone(),
        model,
        128_000, // default context window; TODO: make configurable per-provider
    );

    adapter
        .validate()
        .await
        .map_err(|e| anyhow::anyhow!("Provider validation failed: {}", e))?;

    // Build the tool registry
    let mut tool_registry = niya_core::ToolRegistry::new();
    niya_tools::register_all_with_config(
        &mut tool_registry,
        config.session.shell_timeout,
        config.session.shell_output_limit,
    );

    // Build the permission gate
    let permission_gate =
        niya_core::PermissionGate::new(config.permissions.clone(), &project_root);

    // Build the context manager
    let context_window = adapter.context_window_size();
    let mut context_manager = niya_core::ContextManager::new(context_window);
    context_manager.configure_project_context(
        config.context.project_instruction_file.clone(),
        config.context.max_project_context_lines,
    );
    context_manager.init(&project_root).await?;

    // Build the session logger
    let session_id = uuid::Uuid::new_v4().to_string();
    let log_dir = project_root.join(&config.session.log_directory);
    let session_logger = niya_core::SessionLogger::new(&session_id, Some(&log_dir));

    // Build the orchestrator
    let orch_config = niya_core::OrchestratorConfig {
        max_iterations: config.session.max_iterations,
        dry_run: cli.dry_run,
    };

    let tool_context = niya_core::ToolContext {
        project_root: project_root.clone(),
        cwd: project_root.clone(),
        env: std::env::vars().collect(),
    };

    let orchestrator = niya_core::Orchestrator::new(
        Box::new(adapter),
        tool_registry,
        permission_gate,
        context_manager,
        session_logger,
        orch_config,
        tool_context,
    );

    // Decide between one-shot and interactive mode
    if let Some(prompt) = cli.prompt {
        if cli.print {
            repl::run_oneshot(orchestrator, &prompt).await?;
        } else {
            repl::run_interactive(orchestrator, Some(&prompt), &config).await?;
        }
    } else {
        repl::run_interactive(orchestrator, None, &config).await?;
    }

    Ok(())
}
