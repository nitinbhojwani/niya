//! Nexus Providers — LLM provider adapters.
//!
//! Each adapter translates between Nexus's canonical message format and
//! a specific LLM provider's API.

pub mod openai_compat;

// Re-export adapters.
pub use openai_compat::OpenAiCompatAdapter;
