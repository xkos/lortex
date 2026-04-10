//! Tool registry — dynamic lookup and management of tools.

use std::collections::HashMap;
use std::sync::Arc;

use lortex_core::tool::Tool;

/// A registry that holds named tools for dynamic lookup.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Look up a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Get all registered tools.
    pub fn all(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Remove a tool by name.
    pub fn remove(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.remove(name)
    }

    /// List all tool names.
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
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
    use lortex_core::tool::{FnTool, ToolOutput};
    use serde_json::json;

    fn make_tool(name: &str) -> Arc<dyn Tool> {
        Arc::new(FnTool::new(
            name,
            format!("{name} description"),
            json!({}),
            |_| async { Ok(ToolOutput::text("ok")) },
        ))
    }

    #[test]
    fn new_registry_is_empty() {
        let reg = ToolRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn register_and_get() {
        let mut reg = ToolRegistry::new();
        reg.register(make_tool("search"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());

        let tool = reg.get("search").unwrap();
        assert_eq!(tool.name(), "search");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let reg = ToolRegistry::new();
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn register_overwrites_same_name() {
        let mut reg = ToolRegistry::new();
        reg.register(make_tool("dup"));
        reg.register(make_tool("dup"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn all_returns_all_tools() {
        let mut reg = ToolRegistry::new();
        reg.register(make_tool("a"));
        reg.register(make_tool("b"));
        reg.register(make_tool("c"));

        let all = reg.all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn names_returns_all_names() {
        let mut reg = ToolRegistry::new();
        reg.register(make_tool("alpha"));
        reg.register(make_tool("beta"));

        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn remove_existing_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(make_tool("removeme"));
        assert_eq!(reg.len(), 1);

        let removed = reg.remove("removeme");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name(), "removeme");
        assert!(reg.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut reg = ToolRegistry::new();
        assert!(reg.remove("ghost").is_none());
    }

    #[test]
    fn default_impl() {
        let reg = ToolRegistry::default();
        assert!(reg.is_empty());
    }
}
