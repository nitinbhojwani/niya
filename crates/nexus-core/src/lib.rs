//! Nexus Core — types, traits, and orchestrator for the coding agent.
//!
//! This crate defines the canonical data structures and interfaces that all
//! other Nexus crates depend on. It also contains the orchestrator (agentic
//! loop), context manager, permission gate, and session logger.

pub mod context;
pub mod orchestrator;
pub mod permission;
pub mod provider;
pub mod session;
pub mod tool;
pub mod types;

// Re-export the most commonly used items.
pub use context::ContextManager;
pub use orchestrator::{Orchestrator, OrchestratorConfig};
pub use permission::PermissionGate;
pub use provider::ProviderAdapter;
pub use session::SessionLogger;
pub use tool::{Tool, ToolRegistry};
pub use types::*;
