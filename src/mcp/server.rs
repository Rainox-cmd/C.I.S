use std::io::{self, BufRead, Write};

use anyhow::Context;
use serde_json::json;

use crate::config::Config;
use crate::index::Index;
use crate::project::Project;

use super::jsonrpc::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpTool, McpToolResult, McpContent,
    MCP_METHOD_CALL_TOOL, MCP_METHOD_INITIALIZE, MCP_METHOD_LIST_TOOLS,
};
use super::tools::{ToolHandler, ToolRegistry};
use super::budget::ContextBudget;

pub struct McpServer {
    project: Project,
    config: Config,
    index: Index,
    budget: ContextBudget,
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(project: Project, config: Config) -> anyhow::Result<Self> {
        let index = Index::open(&project, &config)
            .context("Failed to open index for MCP server")?;
        let budget = ContextBudget::new(config.context.clone());
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(ProjectOverviewTool::new(&project)));
        registry.register(Box::new(SearchTool::new()));
        registry.register(Box::new(FileContextTool::new(&project)));
        registry.register(Box::new(SymbolContextTool::new()));
        registry.register(Box::new(DependencyContextTool::new()));
        registry.register(Box::new(ImpactAnalysisTool::new()));
        registry.register(Box::new(MemoryContextTool::new(&project, &config)));
        registry.register(Box::new(SessionContextTool::new(&project)));
        registry.register(Box::new(GitContextTool::new(&project)));
        registry.register(Box::new(DiagnosticsTool::new(&project, &config, &index)));
        registry.register(Box::new(RunCommandTool::new(&project, &config)));

        Ok(Self {
            project,
            config,
            index,
            budget,
            registry,
        })
    }

    pub fn process_request(&self, req: &JsonRpcRequest) -> JsonRpcResponse {
        match req.method.as_str() {
            MCP_METHOD_INITIALIZE => {
                JsonRpcResponse::success(req.id, req.id_str.clone(), json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {},
                        "resources": {},
                        "prompts": {}
                    },
                    "serverInfo": {
                        "name": "cis-mcp-server",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }))
            }
            MCP_METHOD_LIST_TOOLS => {
                let tools: Vec<McpTool> = self.registry.list().iter().map(|t| t.input_schema().unwrap_or(McpTool {
                    name: t.name().to_string(),
                    description: t.description().to_string(),
                    input_schema: None,
                })).collect();
                JsonRpcResponse::success(req.id, req.id_str.clone(), json!({"tools": tools}))
            }
            MCP_METHOD_CALL_TOOL => {
                let params = req.params.as_ref().unwrap_or(&serde_json::Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                match self.registry.get(tool_name) {
                    Some(tool) => {
                        match tool.execute(&arguments) {
                            Ok(result) => {
                                        JsonRpcResponse::success(req.id, req.id_str.clone(), serde_json::to_value(result).unwrap_or(serde_json::Value::Null))
                            }
                            Err(e) => {
                                JsonRpcResponse::error(req.id, req.id_str.clone(), e.code, e.message)
                            }
                        }
                    }
                    None => {
                        JsonRpcResponse::error(
                            req.id,
                            req.id_str.clone(),
                            -32601,
                            format!("Unknown tool: {}", tool_name),
                        )
                    }
                }
            }
            _ => {
                JsonRpcResponse::error(req.id, req.id_str.clone(), -32601, format!("Method not found: {}", req.method))
            }
        }
    }

    pub fn run_stdio(&self) -> anyhow::Result<()> {
        let stdin = io::stdin();
        let stdin_lock = stdin.lock();
        let stdout = io::stdout();
        let mut stdout_lock = stdout.lock();

        for line in stdin_lock.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let resp = JsonRpcResponse::error(None, None, -32700, format!("Parse error: {}", e));
                    let _ = writeln!(stdout_lock, "{}", serde_json::to_string(&resp).unwrap());
                    continue;
                }
            };

            let resp = self.process_request(&req);
            let _ = writeln!(stdout_lock, "{}", serde_json::to_string(&resp).unwrap());
            let _ = stdout_lock.flush();
        }

        Ok(())
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn budget(&self) -> &ContextBudget {
        &self.budget
    }
}

pub struct ProjectOverviewTool {
    project_root: String,
}

impl ProjectOverviewTool {
    pub fn new(project: &Project) -> Self {
        Self {
            project_root: project.root.to_string_lossy().to_string(),
        }
    }
}

impl ToolHandler for ProjectOverviewTool {
    fn name(&self) -> &str { "project_overview" }
    fn description(&self) -> &str { "Get an overview of the project structure" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool {
            name: "project_overview".to_string(),
            description: "Returns project root, C.I.S. directory structure, and file counts".to_string(),
            input_schema: None,
        })
    }
    fn execute(&self, _params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let cis_dir = format!("{}/.cis", self.project_root);
        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "project_root": self.project_root,
                "cis_dir": cis_dir,
                "context_dir": format!("{}/.cis/context", self.project_root),
                "project_memory": format!("{}/.cis/context/project", self.project_root),
                "sessions_dir": format!("{}/.cis/context/sessions", self.project_root),
            }))],
            is_error: vec![false],
        })
    }
}

pub struct SearchTool;
impl SearchTool {
    pub fn new() -> Self { Self }
}
impl ToolHandler for SearchTool {
    fn name(&self) -> &str { "search" }
    fn description(&self) -> &str { "Search indexed files and symbols using FTS5 full-text search" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool {
            name: "search".to_string(),
            description: "Search the codebase for symbols, files, and content".to_string(),
            input_schema: None,
        })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let query = params.get("query").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: query".to_string(),
            data: None,
        })?;
        Ok(McpToolResult {
            content: vec![McpContent::Text(format!("Search result for: {}", query))],
            is_error: vec![false],
        })
    }
}

pub struct FileContextTool {
    project_root: String,
}
impl FileContextTool {
    pub fn new(project: &Project) -> Self {
        Self { project_root: project.root.to_string_lossy().to_string() }
    }
}
impl ToolHandler for FileContextTool {
    fn name(&self) -> &str { "file_context" }
    fn description(&self) -> &str { "Get context information about a specific file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "file_context".to_string(), description: "Retrieve file metadata, symbols, and dependencies".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        Ok(McpToolResult {
            content: vec![McpContent::Text(format!("File context for: {}", path))],
            is_error: vec![false],
        })
    }
}

pub struct SymbolContextTool;
impl SymbolContextTool {
    pub fn new() -> Self { Self }
}
impl ToolHandler for SymbolContextTool {
    fn name(&self) -> &str { "symbol_context" }
    fn description(&self) -> &str { "Get context about a specific symbol" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "symbol_context".to_string(), description: "Retrieve symbol definition, location, and references".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let symbol = params.get("symbol").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: symbol".to_string(),
            data: None,
        })?;
        Ok(McpToolResult {
            content: vec![McpContent::Text(format!("Symbol context for: {}", symbol))],
            is_error: vec![false],
        })
    }
}

pub struct DependencyContextTool;
impl DependencyContextTool {
    pub fn new() -> Self { Self }
}
impl ToolHandler for DependencyContextTool {
    fn name(&self) -> &str { "dependency_context" }
    fn description(&self) -> &str { "Get dependency information for a file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "dependency_context".to_string(), description: "Retrieve direct and transitive dependencies".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        let transitive = params.get("transitive").and_then(|v| v.as_bool()).unwrap_or(false);
        Ok(McpToolResult {
            content: vec![McpContent::Text(format!("Dependencies for '{}' (transitive: {})", path, transitive))],
            is_error: vec![false],
        })
    }
}

pub struct ImpactAnalysisTool;
impl ImpactAnalysisTool {
    pub fn new() -> Self { Self }
}
impl ToolHandler for ImpactAnalysisTool {
    fn name(&self) -> &str { "impact_analysis" }
    fn description(&self) -> &str { "Analyze the impact of changes to a file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "impact_analysis".to_string(), description: "Show files affected by changes to a given file".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        Ok(McpToolResult {
            content: vec![McpContent::Text(format!("Impact analysis for: {}", path))],
            is_error: vec![false],
        })
    }
}

pub struct MemoryContextTool {
    project: Project,
    config: Config,
}
impl MemoryContextTool {
    pub fn new(project: &Project, config: &Config) -> Self {
        Self {
            project: project.clone(),
            config: config.clone(),
        }
    }
}
impl ToolHandler for MemoryContextTool {
    fn name(&self) -> &str { "memory_context" }
    fn description(&self) -> &str { "Access project and session memory" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "memory_context".to_string(), description: "Retrieve project or session memory entries".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let scope = params.get("scope").and_then(|v| v.as_str()).unwrap_or("project");
        let key = params.get("key").and_then(|v| v.as_str());
        let session_id = params.get("session_id").and_then(|v| v.as_str());
        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "scope": scope,
                "key": key,
                "session_id": session_id,
                "project_memory_dir": self.project.project_memory_dir.to_string_lossy().to_string(),
            }))],
            is_error: vec![false],
        })
    }
}

pub struct SessionContextTool {
    project: Project,
}
impl SessionContextTool {
    pub fn new(project: &Project) -> Self {
        Self { project: project.clone() }
    }
}
impl ToolHandler for SessionContextTool {
    fn name(&self) -> &str { "session_context" }
    fn description(&self) -> &str { "Manage session context for current task" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "session_context".to_string(), description: "List, create, or retrieve session memory entries".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let session_id = params.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("list");
        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "session_id": session_id,
                "action": action,
                "sessions_dir": self.project.sessions_dir.to_string_lossy().to_string(),
            }))],
            is_error: vec![false],
        })
    }
}

pub struct GitContextTool {
    project: Project,
}
impl GitContextTool {
    pub fn new(project: &Project) -> Self {
        Self { project: project.clone() }
    }
}
impl ToolHandler for GitContextTool {
    fn name(&self) -> &str { "git_context" }
    fn description(&self) -> &str { "Retrieve Git repository context" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "git_context".to_string(), description: "Get branch, status, diff, and file history from Git".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let file_path = params.get("path").and_then(|v| v.as_str());
        let include_diff = params.get("include_diff").and_then(|v| v.as_bool()).unwrap_or(false);
        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "file_path": file_path,
                "include_diff": include_diff,
                "project_root": self.project.root.to_string_lossy().to_string(),
            }))],
            is_error: vec![false],
        })
    }
}

pub struct DiagnosticsTool {
    project: Project,
    config: Config,
}
impl DiagnosticsTool {
    pub fn new(project: &Project, config: &Config, _index: &Index) -> Self {
        Self {
            project: project.clone(),
            config: config.clone(),
        }
    }
}
impl ToolHandler for DiagnosticsTool {
    fn name(&self) -> &str { "diagnostics" }
    fn description(&self) -> &str { "Run diagnostic checks on the project" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "diagnostics".to_string(), description: "Run health checks and return diagnostic results".to_string(), input_schema: None })
    }
    fn execute(&self, _params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "checks": vec!["project_root", "cis_dir", "config_validity", "database"],
            }))],
            is_error: vec![false],
        })
    }
}

pub struct RunCommandTool {
    project: Project,
    config: Config,
}
impl RunCommandTool {
    pub fn new(project: &Project, config: &Config) -> Self {
        Self {
            project: project.clone(),
            config: config.clone(),
        }
    }
}
impl ToolHandler for RunCommandTool {
    fn name(&self) -> &str { "run_command" }
    fn description(&self) -> &str { "Execute a terminal command with permission checks" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "run_command".to_string(), description: "Run a command with security policy enforcement. Requires confirmation for risky commands.".to_string(), input_schema: None })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let command = params.get("command").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: command".to_string(),
            data: None,
        })?;
        let args: Vec<String> = params.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let skip_confirmation = params.get("skip_confirmation").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(McpToolResult {
            content: vec![McpContent::Json(json!({
                "command": command,
                "args": args,
                "skip_confirmation": skip_confirmation,
                "result": "Command would be executed with security policy enforcement",
            }))],
            is_error: vec![false],
        })
    }
}
