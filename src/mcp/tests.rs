use super::jsonrpc::*;
use super::server::*;
use super::tools::*;
use super::budget::ContextBudget;
use crate::config::{Config, ContextConfig};
use crate::project::Project;
use tempfile::tempdir;
use serde_json::json;

fn setup() -> (tempfile::TempDir, Project) {
    let dir = tempdir().unwrap();
    let project = Project::new(dir.path().to_path_buf()).unwrap();
    project.init().unwrap();
    (dir, project)
}

#[test]
fn test_mcp_jsonrpc_request_parsing() {
    let req: JsonRpcRequest = serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#).unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(1));
    assert_eq!(req.method, "initialize");
}

#[test]
fn test_mcp_jsonrpc_response_success() {
    let resp = JsonRpcResponse::success(Some(1), None, json!({"result": "ok"}));
    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[test]
fn test_mcp_jsonrpc_response_error() {
    let resp = JsonRpcResponse::error(Some(1), None, -32601, "Method not found".to_string());
    assert_eq!(resp.jsonrpc, "2.0");
    assert!(resp.result.is_none());
    assert!(resp.error.is_some());
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}

#[test]
fn test_mcp_protocol_compliance() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(1),
        id_str: None,
        method: "initialize".to_string(),
        params: Some(json!({})),
    };

    let resp = server.process_request(&req);
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(1));
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());

    let result = resp.result.unwrap();
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert!(result["capabilities"]["tools"].is_object());
}

#[test]
fn test_mcp_tool_list() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(2),
        id_str: None,
        method: "tools/list".to_string(),
        params: None,
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result["tools"].is_array());
    let tools = result["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 11);
}

#[test]
fn test_mcp_tool_call_project_overview() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(3),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "project_overview",
            "arguments": {}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result.is_object());
}

#[test]
fn test_mcp_tool_call_search() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(4),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "search",
            "arguments": {"query": "test"}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result.is_object());
}

#[test]
fn test_mcp_tool_call_unknown_tool() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(5),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "nonexistent_tool",
            "arguments": {}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_some());
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}

#[test]
fn test_mcp_permission_enforcement_run_command_blocked() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(6),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "run_command",
            "arguments": {"command": "rm", "args": ["-rf", "/"]}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result.is_object());
}

#[test]
fn test_mcp_context_budget_enforcement() {
    let cfg = ContextConfig {
        max_session_context_bytes: 100,
        max_sessions: 50,
        max_total_context_bytes: 500,
        max_context_file_bytes: 1024 * 1024,
        session_ttl_seconds: 60,
    };
    let mut budget = ContextBudget::new(cfg);

    assert!(budget.can_allocate(50));
    assert!(budget.allocate(50));
    assert!(!budget.can_allocate(60));
    assert!(!budget.allocate(60));
    assert_eq!(budget.usage(), 50);
    assert_eq!(budget.remaining_bytes(), 50);
}

#[test]
fn test_mcp_context_budget_reset() {
    let cfg = ContextConfig {
        max_session_context_bytes: 100,
        max_sessions: 50,
        max_total_context_bytes: 500,
        max_context_file_bytes: 1024 * 1024,
        session_ttl_seconds: 60,
    };
    let mut budget = ContextBudget::new(cfg);

    assert!(budget.allocate(30));
    assert_eq!(budget.usage(), 30);
    budget.reset();
    assert_eq!(budget.usage(), 0);
    assert_eq!(budget.tool_count(), 0);
}

#[test]
fn test_mcp_error_handling_missing_param() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(7),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "search",
            "arguments": {}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_some());
    assert_eq!(resp.error.as_ref().unwrap().code, -32602);
}

#[test]
fn test_mcp_method_not_found() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(8),
        id_str: None,
        method: "unknown_method".to_string(),
        params: None,
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_some());
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}

#[test]
fn test_mcp_all_tools_present() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(9),
        id_str: None,
        method: "tools/list".to_string(),
        params: None,
    };

    let resp = server.process_request(&req);
    let result = resp.result.unwrap();
    let tools = result["tools"].as_array().unwrap();

    let expected_names = [
        "project_overview",
        "search",
        "file_context",
        "symbol_context",
        "dependency_context",
        "impact_analysis",
        "memory_context",
        "session_context",
        "git_context",
        "diagnostics",
        "run_command",
    ];

    for name in &expected_names {
        assert!(
            tools.iter().any(|t| t["name"].as_str() == Some(*name)),
            "Tool '{}' not found in tool list",
            name
        );
    }
}

#[test]
fn test_mcp_tool_registry() {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(ProjectOverviewTool::new(
        &Project::new(std::env::current_dir().unwrap()).unwrap(),
    )));

    assert!(registry.get("project_overview").is_some());
    assert!(registry.get("nonexistent").is_none());
    assert_eq!(registry.list_names().len(), 1);
}

#[test]
fn test_mcp_all_tool_handlers() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project.clone(), cfg.clone()).unwrap();

    let tool_test_args: std::collections::HashMap<&str, serde_json::Value> = [
        ("project_overview", json!({})),
        ("search", json!({"query": "test"})),
        ("file_context", json!({"path": "src/main.rs"})),
        ("symbol_context", json!({"symbol": "main"})),
        ("dependency_context", json!({"path": "src/main.rs"})),
        ("impact_analysis", json!({"path": "src/main.rs"})),
        ("memory_context", json!({"scope": "project"})),
        ("session_context", json!({"session_id": "default"})),
        ("git_context", json!({})),
        ("diagnostics", json!({})),
        ("run_command", json!({"command": "echo", "args": ["hello"]})),
    ].iter().cloned().collect();

    for (name, args) in &tool_test_args {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            id_str: None,
            method: "tools/call".to_string(),
            params: Some(json!({"name": name, "arguments": args})),
        };
        let resp = server.process_request(&req);
        assert!(resp.error.is_none(), "Tool '{}' failed: {:?}", name, resp.error);
        assert!(resp.result.is_some(), "Tool '{}' returned no result", name);
    }
}

#[test]
fn test_mcp_provenance_enum() {
    assert_eq!(format!("{}", crate::memory::Provenance::Detected), "detected");
    assert_eq!(format!("{}", crate::memory::Provenance::Inferred), "inferred");
    assert_eq!(format!("{}", crate::memory::Provenance::AiGenerated), "ai_generated");
    assert_eq!(
        format!("{}", crate::memory::Provenance::DeveloperConfirmed),
        "developer_confirmed"
    );

    assert!("detected".parse::<crate::memory::Provenance>().is_ok());
    assert!("inferred".parse::<crate::memory::Provenance>().is_ok());
    assert!("ai_generated".parse::<crate::memory::Provenance>().is_ok());
    assert!("developer_confirmed".parse::<crate::memory::Provenance>().is_ok());
    assert!("invalid".parse::<crate::memory::Provenance>().is_err());
}

#[test]
fn test_mcp_memory_context_tool() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(10),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "memory_context",
            "arguments": {"scope": "project", "key": "arch"}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[test]
fn test_mcp_git_context_tool() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(11),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "git_context",
            "arguments": {"path": "src/main.rs", "include_diff": false}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[test]
fn test_mcp_diagnostics_tool() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(12),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "diagnostics",
            "arguments": {}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[test]
fn test_mcp_run_command_tool_blocked() {
    let (_dir, project) = setup();
    let cfg = Config::default();
    let server = McpServer::new(project, cfg).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(13),
        id_str: None,
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "run_command",
            "arguments": {"command": "rm", "args": ["-rf", "/"], "skip_confirmation": false}
        })),
    };

    let resp = server.process_request(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result["content"][0]["data"]["result"].as_str().unwrap().contains("security policy"));
}
