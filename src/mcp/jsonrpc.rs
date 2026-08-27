use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_str: Option<String>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_str: Option<String>,
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
        self.id.is_some() || self.id_str.is_some()
    }
}

impl JsonRpcResponse {
    pub fn success(id: Option<u64>, id_str: Option<String>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            id_str,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, id_str: Option<String>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            id_str,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<(String, serde_json::Value)>,
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
#[serde(tag = "type", content = "data")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "json")]
    Json(serde_json::Value),
    #[serde(rename = "file")]
    File(String),
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
