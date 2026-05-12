//! Terminal renderer.
//!
//! Renders output events from the orchestrator to the terminal. Handles
//! streaming tokens, tool call banners, errors, and status messages.
//! Colour output is gated behind a configuration flag.

use crossterm::style::Stylize;
use std::io::{self, Write};

use niya_core::OutputEvent;

/// Renders orchestrator output events to the terminal.
pub struct Renderer {
    color: bool,
    markdown: bool,
}

impl Renderer {
    pub fn new(color: bool, markdown: bool) -> Self {
        Self { color, markdown }
    }

    /// Render a single output event.
    pub fn render(&self, event: &OutputEvent) {
        match event {
            OutputEvent::Token { text } => {
                print!("{}", text);
                let _ = io::stdout().flush();
            }

            OutputEvent::ToolCall { tool_name, args } => {
                self.print_tool_call(tool_name, args);
            }

            OutputEvent::ToolResult { tool_name, result } => {
                self.print_tool_result(tool_name, result);
            }

            OutputEvent::ApprovalRequest {
                tool_name,
                args: _,
                message,
            } => {
                self.print_approval_request(tool_name, message);
            }

            OutputEvent::Status { message } => {
                // Status messages like "Thinking..." are shown inline
                if self.color {
                    eprint!("\r{}", message.clone().dark_grey());
                } else {
                    eprint!("\r{}", message);
                }
                let _ = io::stderr().flush();
            }

            OutputEvent::Error { error } => {
                self.print_error(&error.to_string());
            }

            OutputEvent::Done => {
                // Ensure we end on a new line after streaming tokens
                println!();
            }
        }
    }

    /// Print the input prompt marker.
    pub fn print_prompt(&self) {
        if self.color {
            print!("{} ", "niya>".bold().cyan());
        } else {
            print!("niya> ");
        }
        let _ = io::stdout().flush();
    }

    /// Print the welcome banner.
    pub fn print_welcome(&self) {
        let banner = r#"
  _   _
 | \ | | _____  ___   _ ___
 |  \| |/ _ \ \/ / | | / __|
 | |\  |  __/>  <| |_| \__ \
 |_| \_|\___/_/\_\\__,_|___/
"#;
        if self.color {
            println!("{}", banner.cyan());
            println!(
                "  {}  Type {} or {} to exit.\n",
                "AI coding assistant".dark_grey(),
                "/help".bold(),
                "/quit".bold()
            );
        } else {
            println!("{}", banner);
            println!("  AI coding assistant.  Type /help or /quit to exit.\n");
        }
    }

    /// Print a goodbye message.
    pub fn print_goodbye(&self) {
        println!("\nGoodbye!");
    }

    /// Print help text for slash commands.
    pub fn print_help(&self) {
        let commands = [
            ("/help", "Show this help message"),
            ("/clear", "Clear conversation history"),
            ("/add <path>", "Add a file to the context"),
            ("/usage", "Show token usage statistics"),
            ("/quit", "Exit Niya"),
        ];

        println!("\nAvailable commands:\n");
        for (cmd, desc) in &commands {
            if self.color {
                println!("  {:16} {}", cmd.bold(), desc.dark_grey());
            } else {
                println!("  {:16} {}", cmd, desc);
            }
        }
        println!();
    }

    /// Print a user prompt echo (for initial prompt in interactive mode).
    pub fn print_user_prompt(&self, text: &str) {
        if self.color {
            println!("{} {}", "niya>".bold().cyan(), text);
        } else {
            println!("niya> {}", text);
        }
    }

    /// Print a status message.
    pub fn print_status(&self, message: &str) {
        if self.color {
            println!("{}", message.dark_grey());
        } else {
            println!("{}", message);
        }
    }

    /// Print an error message.
    pub fn print_error(&self, message: &str) {
        if self.color {
            eprintln!("{} {}", "error:".bold().red(), message);
        } else {
            eprintln!("error: {}", message);
        }
    }

    fn print_tool_call(&self, tool_name: &str, args: &serde_json::Value) {
        let summary = summarise_tool_args(tool_name, args);
        if self.color {
            eprintln!(
                "\n{} {} {}",
                "▶".dark_yellow(),
                tool_name.bold(),
                summary.dark_grey()
            );
        } else {
            eprintln!("\n> {} {}", tool_name, summary);
        }
    }

    fn print_tool_result(&self, _tool_name: &str, result: &niya_core::ToolResult) {
        let icon = if result.success { "✔" } else { "✘" };
        let preview = truncate(&result.output, 200);

        if self.color {
            let styled_icon = if result.success {
                icon.green().to_string()
            } else {
                icon.red().to_string()
            };
            eprintln!("  {} {}", styled_icon, preview.dark_grey());
        } else {
            eprintln!("  {} {}", icon, preview);
        }
    }

    fn print_approval_request(&self, _tool_name: &str, message: &str) {
        if self.color {
            eprintln!(
                "\n{} {} {}",
                "⚠".dark_yellow(),
                "Permission required:".bold().dark_yellow(),
                message,
            );
        } else {
            eprintln!("\n! Permission required: {}", message);
        }
    }
}

/// Create a one-line summary of tool arguments for display.
fn summarise_tool_args(tool_name: &str, args: &serde_json::Value) -> String {
    match tool_name {
        "file_read" => args
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "file_write" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let len = args
                .get("content")
                .and_then(|v| v.as_str())
                .map(|c| c.len())
                .unwrap_or(0);
            format!("{} ({} bytes)", path, len)
        }
        "file_edit" => args
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "shell_execute" => args
            .get("command")
            .and_then(|v| v.as_str())
            .map(|c| truncate(c, 80))
            .unwrap_or_default(),
        "grep" => args
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        "glob" => args
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(|p| p.to_string())
            .unwrap_or_default(),
        _ => serde_json::to_string(args).unwrap_or_default(),
    }
}

/// Truncate a string to `max_len` characters, adding "…" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let result = truncate("a very long string that exceeds the limit", 10);
        assert!(result.ends_with('…'));
        assert!(result.len() <= 14); // 10 bytes + up to 3 for "…" in UTF-8
    }

    #[test]
    fn summarise_file_read_shows_path() {
        let args = serde_json::json!({"file_path": "src/main.rs"});
        assert_eq!(summarise_tool_args("file_read", &args), "src/main.rs");
    }

    #[test]
    fn summarise_shell_shows_command() {
        let args = serde_json::json!({"command": "cargo test"});
        assert_eq!(summarise_tool_args("shell_execute", &args), "cargo test");
    }
}
