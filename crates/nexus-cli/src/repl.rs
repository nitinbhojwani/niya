//! REPL (Read-Eval-Print Loop) for interactive sessions.
//!
//! Handles user input, dispatches to the orchestrator, and renders output
//! events via the renderer.

use anyhow::Result;
use std::io::{self, Write};

use nexus_config::ResolvedConfig;
use nexus_core::{Orchestrator, OutputEvent};

use crate::commands::{self, SlashCommand};
use crate::renderer::Renderer;

/// Run a single prompt and print the response (non-interactive mode).
pub async fn run_oneshot(mut orchestrator: Orchestrator, prompt: &str) -> Result<()> {
    let renderer = Renderer::new(false, false);
    let events = orchestrator
        .run(prompt.to_string(), None, &mut None)
        .await;

    for event in &events {
        renderer.render(event);
    }

    Ok(())
}

/// Run the interactive REPL.
pub async fn run_interactive(
    mut orchestrator: Orchestrator,
    initial_prompt: Option<&str>,
    config: &ResolvedConfig,
) -> Result<()> {
    let renderer = Renderer::new(config.display.color, config.display.markdown);

    renderer.print_welcome();

    // If we have an initial prompt, run it first
    if let Some(prompt) = initial_prompt {
        renderer.print_user_prompt(prompt);
        let events = orchestrator
            .run(prompt.to_string(), None, &mut None)
            .await;
        for event in &events {
            renderer.render(event);
        }
    }

    // Main REPL loop
    loop {
        let input = match read_input(&renderer) {
            Ok(Some(line)) => line,
            Ok(None) => break, // EOF / Ctrl-D
            Err(e) => {
                renderer.print_error(&format!("Input error: {}", e));
                continue;
            }
        };

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check for slash commands
        if trimmed.starts_with('/') {
            match commands::parse(trimmed) {
                SlashCommand::Quit => break,
                SlashCommand::Clear => {
                    orchestrator.reset();
                    renderer.print_status("Conversation cleared.");
                    continue;
                }
                SlashCommand::Help => {
                    renderer.print_help();
                    continue;
                }
                SlashCommand::Add { path } => {
                    let p = std::path::Path::new(&path);
                    match orchestrator.add_file_to_context(p).await {
                        Ok(()) => renderer.print_status(&format!("Added {} to context.", path)),
                        Err(e) => renderer.print_error(&format!("Failed to add file: {}", e)),
                    }
                    continue;
                }
                SlashCommand::Usage => {
                    let usage = orchestrator.usage();
                    renderer.print_status(&format!(
                        "Tokens used: {} input, {} output ({} total)",
                        usage.input_tokens,
                        usage.output_tokens,
                        usage.total()
                    ));
                    continue;
                }
                SlashCommand::Unknown { name } => {
                    renderer.print_error(&format!(
                        "Unknown command: /{}. Type /help for available commands.",
                        name
                    ));
                    continue;
                }
            }
        }

        // Regular message — send to orchestrator
        let events = orchestrator
            .run(trimmed.to_string(), None, &mut None)
            .await;

        for event in &events {
            renderer.render(event);
        }
    }

    renderer.print_goodbye();
    Ok(())
}

/// Read a line of input from the user.
///
/// Returns `Ok(None)` on EOF (Ctrl-D), `Ok(Some(line))` on normal input.
fn read_input(renderer: &Renderer) -> Result<Option<String>> {
    renderer.print_prompt();
    io::stdout().flush()?;

    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) => Ok(None), // EOF
        Ok(_) => Ok(Some(line)),
        Err(e) => Err(e.into()),
    }
}
