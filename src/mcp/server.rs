use std::io::{self, BufRead, Write};

use anyhow::Context;
use serde_json::json;

use crate::config::Config;
use crate::index::Index;
use crate::project::Project;

use super::jsonrpc::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpTool, McpToolInputSchema, McpToolResult, McpContent,
    MCP_METHOD_CALL_TOOL, MCP_METHOD_INITIALIZE, MCP_METHOD_LIST_TOOLS,
};
use super::tools::{ToolHandler, ToolRegistry};
use super::budget::ContextBudget;
use std::sync::Mutex;

pub struct McpServer {
    project: Project,
    config: Config,
    index: Index,
    budget: Mutex<ContextBudget>,
    registry: ToolRegistry,
}

impl McpServer {
    pub fn new(project: Project, config: Config) -> anyhow::Result<Self> {
        let index = Index::open(&project, &config)
            .context("Failed to open index for MCP server")?;
        let budget = Mutex::new(ContextBudget::new(config.context.clone()));
        let mut registry = ToolRegistry::new();

        registry.register(Box::new(ProjectOverviewTool::new(&project)));
        registry.register(Box::new(SearchTool::new(&project, &config)));
        registry.register(Box::new(FileContextTool::new(&project, &config)));
        registry.register(Box::new(SymbolContextTool::new(&project, &config)));
        registry.register(Box::new(DependencyContextTool::new(&project, &config)));
        registry.register(Box::new(ImpactAnalysisTool::new(&project, &config)));
        registry.register(Box::new(MemoryContextTool::new(&project, &config)));
        registry.register(Box::new(SessionContextTool::new(&project, &config)));
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
                JsonRpcResponse::success(req.id.clone(), json!({
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
                JsonRpcResponse::success(req.id.clone(), json!({"tools": tools}))
            }
            MCP_METHOD_CALL_TOOL => {
                let params = req.params.as_ref().unwrap_or(&serde_json::Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                match self.registry.get(tool_name) {
                    Some(tool) => {
                        match tool.execute(&arguments) {
                            Ok(result) => {
                                let result_val = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
                                let result_str = serde_json::to_string(&result_val).unwrap_or_default();
                                let size = result_str.len() as u64;

                                let mut budget = self.budget.lock().unwrap();
                                if !budget.allocate(size) {
                                    return JsonRpcResponse::error(
                                        req.id.clone(),
                                        -32603,
                                        format!("Context budget exceeded. Payload size {} bytes exceeds remaining budget {} bytes", size, budget.remaining_bytes())
                                    );
                                }
                                
                                JsonRpcResponse::success(req.id.clone(), result_val)
                            }
                            Err(e) => {
                                JsonRpcResponse::error(req.id.clone(), e.code, e.message)
                            }
                        }
                    }
                    None => {
                        JsonRpcResponse::error(
                            req.id.clone(),
                            -32601,
                            format!("Unknown tool: {}", tool_name),
                        )
                    }
                }
            }
            _ => {
                JsonRpcResponse::error(req.id.clone(), -32601, format!("Method not found: {}", req.method))
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
            
            tracing::info!("MCP Request: {}", line);

            let req: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                    let resp_str = serde_json::to_string(&resp).unwrap();
                    tracing::error!("MCP Parse Error: {} -> {}", e, resp_str);
                    let _ = writeln!(stdout_lock, "{}", resp_str);
                    continue;
                }
            };

            let is_notification = req.id.is_none();
            let resp = self.process_request(&req);
            
            if !is_notification {
                let resp_str = serde_json::to_string(&resp).unwrap();
                tracing::info!("MCP Response: {}", resp_str);
                let _ = writeln!(stdout_lock, "{}", resp_str);
                let _ = stdout_lock.flush();
            } else {
                tracing::info!("MCP Notification received and ignored: {}", req.method);
            }
        }

        Ok(())
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn budget(&self) -> &Mutex<ContextBudget> {
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
            input_schema: Some(McpToolInputSchema {
                schema_type: "object".to_string(),
                properties: std::collections::HashMap::new(),
                required: vec![],
            }),
        })
    }
    fn execute(&self, _params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let cis_dir = format!("{}/.cis", self.project_root);
        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
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

pub struct SearchTool {
    project: Project,
    config: Config,
}
impl SearchTool {
    pub fn new(project: &Project, config: &Config) -> Self { Self { project: project.clone(), config: config.clone() } }
}
impl ToolHandler for SearchTool {
    fn name(&self) -> &str { "search" }
    fn description(&self) -> &str { "Search indexed files and symbols using FTS5 full-text search" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool {
            name: "search".to_string(),
            description: "Search the codebase for symbols, files, and content".to_string(),
            input_schema: Some(McpToolInputSchema {
                schema_type: "object".to_string(),
                properties: {
                    let mut props = std::collections::HashMap::new();
                    props.insert("query".to_string(), serde_json::json!({ "type": "string", "description": "Search query string" }));
                    props
                },
                required: vec!["query".to_string()],
            }),
        })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let query = params.get("query").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: query".to_string(),
            data: None,
        })?;
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;
        let results = index.search(query).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Search failed: {}", e),
            data: None,
        })?;
        
        let json_results: Vec<_> = results.into_iter().map(|r| json!({
            "rel_path": r.rel_path,
            "name": r.name,
            "symbol_type": r.symbol_type,
            "line": r.line,
            "rank": r.rank
        })).collect();

        Ok(McpToolResult {
            content: vec![McpContent::json(json!(json_results))],
            is_error: vec![false],
        })
    }
}

pub struct FileContextTool {
    project: Project,
    config: Config,
}
impl FileContextTool {
    pub fn new(project: &Project, config: &Config) -> Self {
        Self { project: project.clone(), config: config.clone() }
    }
}
impl ToolHandler for FileContextTool {
    fn name(&self) -> &str { "file_context" }
    fn description(&self) -> &str { "Get context information about a specific file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "file_context".to_string(), description: "Retrieve file metadata, symbols, and dependencies".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("path".to_string(), serde_json::json!({ "type": "string", "description": "File path" })); props }, required: vec!["path".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;

        let symbols = index.get_symbols_by_file(path).unwrap_or_default();
        let dependencies = index.get_dependencies(path).unwrap_or_default();
        
        let abs_path = self.project.root.join(path);
        let content = std::fs::read_to_string(&abs_path).unwrap_or_else(|_| "".to_string());
        
        let json_symbols: Vec<_> = symbols.into_iter().map(|s| json!({
            "name": s.name,
            "type": s.symbol_type,
            "line": s.line,
            "column": s.column
        })).collect();
        
        let json_deps: Vec<_> = dependencies.into_iter().map(|d| d.target_file).collect();

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "path": path,
                "symbols": json_symbols,
                "dependencies": json_deps,
                "content_preview": content.chars().take(1000).collect::<String>()
            }))],
            is_error: vec![false],
        })
    }
}

pub struct SymbolContextTool {
    project: Project,
    config: Config,
}
impl SymbolContextTool {
    pub fn new(project: &Project, config: &Config) -> Self { Self { project: project.clone(), config: config.clone() } }
}
impl ToolHandler for SymbolContextTool {
    fn name(&self) -> &str { "symbol_context" }
    fn description(&self) -> &str { "Get context about a specific symbol" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "symbol_context".to_string(), description: "Retrieve symbol definition, location, and references".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("symbol".to_string(), serde_json::json!({ "type": "string", "description": "Symbol name" })); props }, required: vec!["symbol".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let symbol = params.get("symbol").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: symbol".to_string(),
            data: None,
        })?;
        
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;

        let symbols = index.find_symbols_by_name(symbol).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to find symbols: {}", e),
            data: None,
        })?;

        let json_symbols: Vec<_> = symbols.into_iter().map(|s| json!({
            "name": s.name,
            "type": s.symbol_type,
            "file": s.rel_path,
            "line": s.line,
            "column": s.column
        })).collect();

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "symbol": symbol,
                "locations": json_symbols
            }))],
            is_error: vec![false],
        })
    }
}

pub struct DependencyContextTool {
    project: Project,
    config: Config,
}
impl DependencyContextTool {
    pub fn new(project: &Project, config: &Config) -> Self { Self { project: project.clone(), config: config.clone() } }
}
impl ToolHandler for DependencyContextTool {
    fn name(&self) -> &str { "dependency_context" }
    fn description(&self) -> &str { "Get dependency information for a file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "dependency_context".to_string(), description: "Retrieve direct and transitive dependencies".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("path".to_string(), serde_json::json!({ "type": "string", "description": "File path" })); props }, required: vec!["path".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        let transitive = params.get("transitive").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;

        let deps = if transitive {
            index.get_transitive_dependencies(path).unwrap_or_default()
        } else {
            index.get_dependencies(path).unwrap_or_default().into_iter().map(|d| d.target_file).collect()
        };

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "path": path,
                "transitive": transitive,
                "dependencies": deps
            }))],
            is_error: vec![false],
        })
    }
}

pub struct ImpactAnalysisTool {
    project: Project,
    config: Config,
}
impl ImpactAnalysisTool {
    pub fn new(project: &Project, config: &Config) -> Self { Self { project: project.clone(), config: config.clone() } }
}
impl ToolHandler for ImpactAnalysisTool {
    fn name(&self) -> &str { "impact_analysis" }
    fn description(&self) -> &str { "Analyze the impact of changes to a file" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "impact_analysis".to_string(), description: "Show files affected by changes to a given file".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("path".to_string(), serde_json::json!({ "type": "string", "description": "File path" })); props }, required: vec!["path".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let path = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: "Missing required parameter: path".to_string(),
            data: None,
        })?;
        
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;

        let impact = index.get_reverse_dependencies(path).unwrap_or_default();

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "path": path,
                "affected_files": impact
            }))],
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
        Some(McpTool { name: "memory_context".to_string(), description: "Retrieve project or session memory entries".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("scope".to_string(), serde_json::json!({ "type": "string", "enum": ["project", "session"] }));
        props.insert("key".to_string(), serde_json::json!({ "type": "string" })); props }, required: vec!["scope".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let scope = params.get("scope").and_then(|v| v.as_str()).unwrap_or("project");
        let key = params.get("key").and_then(|v| v.as_str());
        let session_id = params.get("session_id").and_then(|v| v.as_str());
        
        let mm = crate::memory::MemoryManager::new(&self.project, &self.config.context).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Memory manager error: {}", e),
            data: None,
        })?;

        let result = if scope == "session" {
            if let (Some(sid), Some(k)) = (session_id, key) {
                let val = mm.session_get(sid, k).unwrap_or(None);
                json!({"entry": val})
            } else if let Some(sid) = session_id {
                let list = mm.session_list(sid).unwrap_or_default();
                json!({"entries": list})
            } else {
                json!({"error": "session_id required for session scope"})
            }
        } else {
            if let Some(k) = key {
                let val = mm.project_get(k).unwrap_or(None);
                json!({"entry": val})
            } else {
                let list = mm.project_list().unwrap_or_default();
                json!({"entries": list})
            }
        };

        Ok(McpToolResult {
            content: vec![McpContent::json(result)],
            is_error: vec![false],
        })
    }
}

pub struct SessionContextTool {
    project: Project,
    config: Config,
}
impl SessionContextTool {
    pub fn new(project: &Project, config: &Config) -> Self {
        Self { project: project.clone(), config: config.clone() }
    }
}
impl ToolHandler for SessionContextTool {
    fn name(&self) -> &str { "session_context" }
    fn description(&self) -> &str { "Manage session context for current task" }
    fn input_schema(&self) -> Option<McpTool> {
        Some(McpTool { name: "session_context".to_string(), description: "List, create, or retrieve session memory entries".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("session_id".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("action".to_string(), serde_json::json!({ "type": "string", "enum": ["list", "get", "set", "delete"] }));
        props.insert("key".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("value".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("category".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("ttl".to_string(), serde_json::json!({ "type": "number" })); props }, required: vec!["session_id".to_string(), "action".to_string()] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let session_id = params.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("list");
        
        let mm = crate::memory::MemoryManager::new(&self.project, &self.config.context).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Memory manager error: {}", e),
            data: None,
        })?;

        let result = match action {
            "list" => {
                let list = mm.session_list(session_id).unwrap_or_default();
                json!({"entries": list})
            }
            "get" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let entry = mm.session_get(session_id, key).unwrap_or(None);
                json!({"entry": entry})
            }
            "set" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let ttl = params.get("ttl").and_then(|v| v.as_u64());
                let provenance = crate::memory::Provenance::AiGenerated;
                let tags = Vec::new();
                mm.session_set(session_id, key, value, provenance, ttl, tags).unwrap_or(());
                json!({"success": true})
            }
            "delete" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let deleted = mm.session_delete(session_id, key).unwrap_or(false);
                json!({"deleted": deleted})
            }
            _ => json!({"error": format!("Unknown action: {}", action)})
        };

        Ok(McpToolResult {
            content: vec![McpContent::json(result)],
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
        Some(McpTool { name: "git_context".to_string(), description: "Get branch, status, diff, and file history from Git".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("path".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("include_diff".to_string(), serde_json::json!({ "type": "boolean" })); props }, required: vec![] }) })
    }
    fn execute(&self, params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let file_path = params.get("path").and_then(|v| v.as_str());
        let include_diff = params.get("include_diff").and_then(|v| v.as_bool()).unwrap_or(false);
        
        let git = crate::git::GitClient::new(&self.project.root).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Git init error: {}", e),
            data: None,
        })?;

        if !git.is_repo() {
            return Ok(McpToolResult {
                content: vec![McpContent::json(json!({"error": "Not a git repository"}))],
                is_error: vec![true],
            });
        }

        let mut result = json!({});
        
        if let Some(path) = file_path {
            if let Ok(history) = git.file_history(path, 10) {
                result["history"] = json!(history.into_iter().map(|c| json!({
                    "hash": c.short_hash,
                    "author": c.author,
                    "date": c.date,
                    "message": c.message
                })).collect::<Vec<_>>());
            }
        } else {
            result["branch"] = json!(git.current_branch().unwrap_or_default());
            result["status"] = json!(git.status().unwrap_or_default().into_iter().map(|s| json!({"path": s.path, "status": s.status})).collect::<Vec<_>>());
            if include_diff {
                result["diff"] = json!(git.diff().unwrap_or_default());
            }
        }

        Ok(McpToolResult {
            content: vec![McpContent::json(result)],
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
        Some(McpTool { name: "diagnostics".to_string(), description: "Run health checks and return diagnostic results".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: std::collections::HashMap::new(), required: vec![] }) })
    }
    fn execute(&self, _params: &serde_json::Value) -> Result<McpToolResult, JsonRpcError> {
        let index = Index::open(&self.project, &self.config).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Failed to open index: {}", e),
            data: None,
        })?;
        
        let report = crate::diagnostics::run_all_checks(&self.project, &self.config, &index);
        
        let is_healthy = report.is_healthy();
        let has_warnings = report.has_warnings();
        
        let checks: Vec<_> = report.checks.into_iter().map(|c| json!({
            "name": c.name,
            "health": match c.health {
                crate::diagnostics::Health::Pass => "PASS",
                crate::diagnostics::Health::Warn => "WARN",
                crate::diagnostics::Health::Fail => "FAIL",
            },
            "message": c.message,
            "details": c.details
        })).collect();

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "is_healthy": is_healthy,
                "has_warnings": has_warnings,
                "checks": checks,
            }))],
            is_error: vec![!is_healthy],
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
        Some(McpTool { name: "run_command".to_string(), description: "Run a command with security policy enforcement. Requires confirmation for risky commands.".to_string(), input_schema: Some(McpToolInputSchema { schema_type: "object".to_string(), properties: { let mut props = std::collections::HashMap::new(); props.insert("command".to_string(), serde_json::json!({ "type": "string" }));
        props.insert("args".to_string(), serde_json::json!({ "type": "array", "items": { "type": "string" } }));
        props.insert("skip_confirmation".to_string(), serde_json::json!({ "type": "boolean" })); props }, required: vec!["command".to_string()] }) })
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

        let executor = crate::terminal::executor::TerminalExecutor::from_config(
            self.project.root.clone(),
            self.project.logs_dir.clone(),
            &self.config
        );

        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let result = executor.execute(command, &str_args, None, skip_confirmation).map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Command execution failed: {}", e),
            data: None,
        })?;

        let is_err = result.blocked || result.requires_confirmation || result.exit_code != 0 || result.timed_out;

        Ok(McpToolResult {
            content: vec![McpContent::json(json!({
                "command": result.command,
                "args": result.args,
                "exit_code": result.exit_code,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "blocked": result.blocked,
                "requires_confirmation": result.requires_confirmation,
                "timed_out": result.timed_out,
                "block_reason": result.block_reason,
            }))],
            is_error: vec![is_err],
        })
    }
}
