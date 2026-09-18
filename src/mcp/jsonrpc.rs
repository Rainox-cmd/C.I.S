use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn has_valid_id(&self) -> bool {
        self.id.is_some()
    }
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "inputSchema")]
    pub input_schema: Option<McpToolInputSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "required")]
    pub required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<McpContent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub is_error: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
}

impl McpContent {
    pub fn json(v: serde_json::Value) -> Self {
        McpContent::Text {
            text: serde_json::to_string(&v).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    pub name: String,
    pub description: String,
}

pub const MCP_METHOD_INITIALIZE: &str = "initialize";
pub const MCP_METHOD_LIST_TOOLS: &str = "tools/list";
pub const MCP_METHOD_CALL_TOOL: &str = "tools/call";
pub const MCP_METHOD_LIST_RESOURCES: &str = "resources/list";
pub const MCP_METHOD_READ_RESOURCE: &str = "resources/read";
pub const MCP_METHOD_LIST_PROMPTS: &str = "prompts/list";
pub const MCP_METHOD_GET_PROMPT: &str = "prompts/get";
pub const MCP_METHOD_NOTIFICATION: &str = "notifications/message";
