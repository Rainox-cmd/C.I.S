use std::collections::HashMap;

use super::jsonrpc::{JsonRpcError, McpTool, McpToolResult};

pub trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Option<McpTool>;
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError>;
}

pub struct ToolRegistry {
    pub tools: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn ToolHandler>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn ToolHandler> {
        self.tools.get(name).map(|b| b.as_ref())
    }

    pub fn list(&self) -> Vec<&dyn ToolHandler> {
        self.tools.values().map(|b| b.as_ref()).collect()
    }

    pub fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
