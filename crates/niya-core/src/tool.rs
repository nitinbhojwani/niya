//! Tool trait and registry.
//!
//! Every capability the agent can invoke (file read, shell execute, grep, etc.)
//! implements the `Tool` trait. The `ToolRegistry` collects all available tools
//! and provides lookup by name.

use async_trait::async_trait;

use crate::types::{ToolContext, ToolResult, ToolSchema};

/// A tool that the agent can invoke.
///
/// Each tool declares a schema (name, description, JSON Schema for parameters)
/// and an async execute function. Implementations must be `Send + Sync`.
#[async_trait]
pub trait Tool: Send + Sync {
    /// The tool's schema, sent to the LLM so it knows what tools are available.
    fn schema(&self) -> &ToolSchema;

    /// Execute the tool with the given input arguments and context.
    async fn execute(
        &self,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> ToolResult;
}

/// Registry of all available tools.
///
/// The orchestrator uses this to look up tools by name when the provider
/// emits a `tool_use` block, and to collect all schemas for the provider request.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Register a tool. Panics if a tool with the same name already exists.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.schema().name.clone();
        if self.get(&name).is_some() {
            panic!("Duplicate tool registration: {}", name);
        }
        self.tools.push(tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.schema().name == name)
            .map(|t| t.as_ref())
    }

    /// Return schemas for all registered tools.
    pub fn all_schemas(&self) -> Vec<ToolSchema> {
        self.tools.iter().map(|t| t.schema().clone()).collect()
    }

    /// Return the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyTool {
        schema: ToolSchema,
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn schema(&self) -> &ToolSchema {
            &self.schema
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            _context: &ToolContext,
        ) -> ToolResult {
            ToolResult::ok("dummy result")
        }
    }

    fn make_dummy(name: &str) -> Box<dyn Tool> {
        Box::new(DummyTool {
            schema: ToolSchema {
                name: name.to_string(),
                description: format!("A dummy {} tool", name),
                parameters: serde_json::json!({"type": "object"}),
            },
        })
    }

    #[test]
    fn registry_registers_and_retrieves_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(make_dummy("file_read"));
        registry.register(make_dummy("grep"));

        assert_eq!(registry.len(), 2);
        assert!(registry.get("file_read").is_some());
        assert!(registry.get("grep").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn registry_returns_all_schemas() {
        let mut registry = ToolRegistry::new();
        registry.register(make_dummy("file_read"));
        registry.register(make_dummy("shell_execute"));

        let schemas = registry.all_schemas();
        assert_eq!(schemas.len(), 2);
        let names: Vec<_> = schemas.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"shell_execute"));
    }

    #[test]
    #[should_panic(expected = "Duplicate tool registration")]
    fn registry_panics_on_duplicate() {
        let mut registry = ToolRegistry::new();
        registry.register(make_dummy("file_read"));
        registry.register(make_dummy("file_read")); // should panic
    }
}
