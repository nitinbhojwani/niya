//! Nexus Tools — concrete tool implementations.
//!
//! Each tool implements the `Tool` trait from nexus-core and provides a
//! capability the agent can invoke: reading files, writing files, editing,
//! running shell commands, searching by glob, and grepping.

pub mod file_read;
pub mod file_write;
pub mod file_edit;
pub mod shell;
pub mod glob_tool;
pub mod grep_tool;
pub mod path_safety;

use nexus_core::tool::ToolRegistry;

/// Register all MVP tools with the given registry.
pub fn register_all(registry: &mut ToolRegistry) {
    register_all_with_config(registry, 30_000, 100_000);
}

/// Register all MVP tools with shell runtime configuration.
pub fn register_all_with_config(
    registry: &mut ToolRegistry,
    shell_timeout_ms: u64,
    shell_output_limit: usize,
) {
    registry.register(Box::new(file_read::FileReadTool::new()));
    registry.register(Box::new(file_write::FileWriteTool::new()));
    registry.register(Box::new(file_edit::FileEditTool::new()));
    registry.register(Box::new(shell::ShellExecuteTool::with_limits(
        shell_timeout_ms,
        shell_output_limit,
    )));
    registry.register(Box::new(glob_tool::GlobTool::new()));
    registry.register(Box::new(grep_tool::GrepTool::new()));
}
