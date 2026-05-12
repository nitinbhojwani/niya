//! Slash command parsing.
//!
//! Parses user input that starts with `/` into structured commands.
//! Unknown commands are preserved so the REPL can report them.

/// A parsed slash command.
#[derive(Debug, PartialEq)]
pub enum SlashCommand {
    /// Exit the REPL.
    Quit,

    /// Clear conversation history.
    Clear,

    /// Show help.
    Help,

    /// Add a file to the context.
    Add { path: String },

    /// Show token usage.
    Usage,

    /// Unrecognised command.
    Unknown { name: String },
}

/// Parse a slash command from user input.
///
/// Expects `input` to start with `/`.
pub fn parse(input: &str) -> SlashCommand {
    let input = input.trim();
    let without_slash = &input[1..]; // skip the leading /
    let mut parts = without_slash.splitn(2, char::is_whitespace);

    let name = parts.next().unwrap_or("").to_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    match name.as_str() {
        "quit" | "q" | "exit" => SlashCommand::Quit,
        "clear" | "reset" => SlashCommand::Clear,
        "help" | "h" | "?" => SlashCommand::Help,
        "add" => {
            if rest.is_empty() {
                SlashCommand::Unknown {
                    name: "add (missing path argument)".to_string(),
                }
            } else {
                SlashCommand::Add {
                    path: rest.to_string(),
                }
            }
        }
        "usage" | "tokens" => SlashCommand::Usage,
        other => SlashCommand::Unknown {
            name: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit_variants() {
        assert_eq!(parse("/quit"), SlashCommand::Quit);
        assert_eq!(parse("/q"), SlashCommand::Quit);
        assert_eq!(parse("/exit"), SlashCommand::Quit);
    }

    #[test]
    fn parse_clear_variants() {
        assert_eq!(parse("/clear"), SlashCommand::Clear);
        assert_eq!(parse("/reset"), SlashCommand::Clear);
    }

    #[test]
    fn parse_help_variants() {
        assert_eq!(parse("/help"), SlashCommand::Help);
        assert_eq!(parse("/h"), SlashCommand::Help);
        assert_eq!(parse("/?"), SlashCommand::Help);
    }

    #[test]
    fn parse_add_with_path() {
        assert_eq!(
            parse("/add src/main.rs"),
            SlashCommand::Add {
                path: "src/main.rs".to_string()
            }
        );
    }

    #[test]
    fn parse_add_without_path_is_unknown() {
        match parse("/add") {
            SlashCommand::Unknown { name } => assert!(name.contains("missing")),
            _ => panic!("Expected Unknown for /add without path"),
        }
    }

    #[test]
    fn parse_usage() {
        assert_eq!(parse("/usage"), SlashCommand::Usage);
        assert_eq!(parse("/tokens"), SlashCommand::Usage);
    }

    #[test]
    fn parse_unknown_command() {
        assert_eq!(
            parse("/foobar"),
            SlashCommand::Unknown {
                name: "foobar".to_string()
            }
        );
    }

    #[test]
    fn parse_handles_whitespace() {
        assert_eq!(parse("  /quit  "), SlashCommand::Quit);
        assert_eq!(
            parse("/add   src/lib.rs  "),
            SlashCommand::Add {
                path: "src/lib.rs".to_string()
            }
        );
    }
}
