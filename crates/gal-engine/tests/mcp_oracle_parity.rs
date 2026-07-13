//! MCP serializer oracle parity tests
//!
//! Verifies that the Rust MCP serializers produce output that matches
//! the frozen PowerShell oracle for all four providers:
//! - Claude Desktop (JSON)
//! - Copilot CLI (JSON)
//! - Codex CLI (TOML)
//! - OpenCode (JSON, mcp.* section)
//!
//! Codex + OpenCode added to reach four-host coverage.

use mcp::serializers::claude_mcp::ClaudeDesktopMcpConfig;
use mcp::serializers::codex::CodexMcpConfig;
use mcp::serializers::copilot::CopilotCliMcpConfig;
use mcp::serializers::opencode::OpenCodeMcpConfig;
use mcp::serializers::McpHostConfig;
use mcp::{McpManifest, McpServer};
use serde_json::Value;
use std::collections::HashMap;

// ─── Claude Desktop ──────────────────────────────────────────────────────────

#[test]
fn test_claude_desktop_simple_stdio_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "test-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string()]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    assert!(value.get("mcpServers").is_some());
    let servers = value["mcpServers"].as_object().unwrap();
    assert_eq!(servers.len(), 1);
    let entry = &servers["test-server"];
    assert_eq!(entry["command"], "node");
    assert_eq!(entry["args"], serde_json::json!(["index.js"]));
}

#[test]
#[cfg(target_os = "windows")]
fn test_claude_desktop_stdio_with_env_windows() {
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "secret123".to_string());
    env.insert("PATH".to_string(), "/usr/bin".to_string());

    let mut servers = HashMap::new();
    servers.insert(
        "test-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string(), "--verbose".to_string()]),
            env: Some(env),
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["test-server"];

    assert_eq!(entry["command"], "powershell");
    assert_eq!(entry["args"][0], "-NoProfile");
    assert_eq!(entry["args"][1], "-Command");
    let script = entry["args"][2].as_str().unwrap();
    assert!(script.contains("$env:API_KEY = 'secret123'"));
    assert!(script.contains("$env:PATH = '/usr/bin'"));
    assert!(script.contains("& 'node' 'index.js' '--verbose'"));
}

#[test]
fn test_claude_desktop_skip_http_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "http-server".to_string(),
        McpServer {
            server_type: Some("http".to_string()),
            command: None,
            args: None,
            env: None,
            url: Some("http://example.com".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["mcpServers"].as_object().unwrap().len(), 0);
}

#[test]
fn test_claude_desktop_skip_url_only_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "url-only".to_string(),
        McpServer {
            server_type: None,
            command: None,
            args: None,
            env: None,
            url: Some("http://example.com".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["mcpServers"].as_object().unwrap().len(), 0);
}

#[test]
fn test_claude_desktop_skip_unresolved_secret() {
    let mut servers = HashMap::new();
    servers.insert(
        "secret-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("${API_KEY}".to_string()),
            args: None,
            env: None,
            url: None,
            headers: None,
        },
    );
    servers.insert(
        "secret-env".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string()]),
            env: {
                let mut env = HashMap::new();
                env.insert("SECRET_KEY".to_string(), "${SECRET}".to_string());
                Some(env)
            },
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["mcpServers"].as_object().unwrap().len(), 0);
}

#[test]
#[cfg(target_os = "windows")]
fn test_claude_desktop_single_quote_escaping() {
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "C:\\Program's Files".to_string());

    let mut servers = HashMap::new();
    servers.insert(
        "test".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["'quoted'".to_string()]),
            env: Some(env),
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["test"];
    let script = entry["args"][2].as_str().unwrap();

    assert!(script.contains("$env:PATH = 'C:\\Program''s Files'"));
    assert!(script.contains("& 'node' '''quoted'''"));
}

// ─── Copilot CLI ─────────────────────────────────────────────────────────────

#[test]
fn test_copilot_cli_local_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "test-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string()]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["test-server"];

    assert_eq!(entry["type"], "local");
    assert_eq!(entry["tools"], serde_json::json!(["*"]));
    assert_eq!(entry["command"], "node");
    assert_eq!(entry["args"], serde_json::json!(["index.js"]));
}

#[test]
fn test_copilot_cli_local_with_env() {
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "secret123".to_string());

    let mut servers = HashMap::new();
    servers.insert(
        "test-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string()]),
            env: Some(env),
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["test-server"];

    assert_eq!(entry["type"], "local");
    assert!(entry.get("env").is_some());
    assert_eq!(entry["env"]["API_KEY"], "secret123");
}

#[test]
fn test_copilot_cli_http_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "http-server".to_string(),
        McpServer {
            server_type: Some("http".to_string()),
            command: None,
            args: None,
            env: None,
            url: Some("http://example.com".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["http-server"];

    assert_eq!(entry["type"], "http");
    assert_eq!(entry["tools"], serde_json::json!(["*"]));
    assert_eq!(entry["url"], "http://example.com");
    assert!(entry.get("command").is_none());
    assert!(entry.get("args").is_none());
}

#[test]
fn test_copilot_cli_sse_server() {
    let mut servers = HashMap::new();
    servers.insert(
        "sse-server".to_string(),
        McpServer {
            server_type: Some("sse".to_string()),
            command: None,
            args: None,
            env: None,
            url: Some("http://example.com/sse".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["sse-server"];

    assert_eq!(entry["type"], "sse");
    assert_eq!(entry["url"], "http://example.com/sse");
}

#[test]
fn test_copilot_cli_infer_transport_from_url() {
    let mut servers = HashMap::new();
    servers.insert(
        "inferred".to_string(),
        McpServer {
            server_type: None,
            command: None,
            args: None,
            env: None,
            url: Some("http://example.com".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["inferred"];
    assert_eq!(entry["type"], "http");
}

#[test]
fn test_copilot_cli_infer_transport_from_command() {
    let mut servers = HashMap::new();
    servers.insert(
        "inferred".to_string(),
        McpServer {
            server_type: None,
            command: Some("node".to_string()),
            args: Some(vec!["index.js".to_string()]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    let entry = &value["mcpServers"]["inferred"];
    assert_eq!(entry["type"], "local");
}

#[test]
fn test_copilot_cli_skip_unresolved_secret() {
    let mut servers = HashMap::new();
    servers.insert(
        "secret-server".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("${API_KEY}".to_string()),
            args: None,
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
    let json = config.to_config_string().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["mcpServers"].as_object().unwrap().len(), 0);
}

// ─── Codex CLI (TOML) — parity ─────────────────────────────────────────

#[test]
fn test_codex_stdio_server_produces_toml_section() {
    // Oracle: ConvertTo-CodexMcpSection for stdio server
    let mut servers = HashMap::new();
    servers.insert(
        "memory".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("npx".to_string()),
            args: Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-memory".to_string(),
            ]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CodexMcpConfig::from_manifest(&manifest).unwrap();
    let toml = config.to_config_string().unwrap();

    assert!(
        toml.contains("[mcp_servers.memory]"),
        "section header missing: {toml}"
    );
    assert!(
        toml.contains("command = \"npx\""),
        "command missing: {toml}"
    );
    assert!(
        toml.contains("args = [\"-y\", \"@modelcontextprotocol/server-memory\"]"),
        "args missing: {toml}"
    );
}

#[test]
fn test_codex_env_vars_produce_nested_section() {
    // Oracle: ConvertTo-CodexMcpSection nests env as [mcp_servers.name.env]
    let mut env = HashMap::new();
    env.insert(
        "MEMORY_FILE_PATH".to_string(),
        "/home/user/mcp-memory.json".to_string(),
    );
    let mut servers = HashMap::new();
    servers.insert(
        "memory".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "@mcp/server-memory".to_string()]),
            env: Some(env),
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CodexMcpConfig::from_manifest(&manifest).unwrap();
    let toml = config.to_config_string().unwrap();

    assert!(
        toml.contains("[mcp_servers.memory.env]"),
        "env section missing: {toml}"
    );
    assert!(
        toml.contains("MEMORY_FILE_PATH = \"/home/user/mcp-memory.json\""),
        "env value missing: {toml}"
    );
}

#[test]
fn test_codex_http_server_produces_type_and_url() {
    // Oracle: ConvertTo-CodexMcpConfig for http type
    let mut servers = HashMap::new();
    servers.insert(
        "remote".to_string(),
        McpServer {
            server_type: Some("http".to_string()),
            command: None,
            args: None,
            env: None,
            url: Some("https://api.example.com/mcp/".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CodexMcpConfig::from_manifest(&manifest).unwrap();
    let toml = config.to_config_string().unwrap();

    assert!(
        toml.contains("[mcp_servers.remote]"),
        "section header missing"
    );
    assert!(toml.contains("type = \"http\""), "type missing: {toml}");
    assert!(
        toml.contains("url = \"https://api.example.com/mcp/\""),
        "url missing: {toml}"
    );
    // stdio fields must not appear for http type
    assert!(
        !toml.contains("command"),
        "command must not appear for http type"
    );
}

#[test]
fn test_codex_special_name_is_quoted() {
    // Oracle: Format-TomlKeySegment quotes names containing '/'
    let mut servers = HashMap::new();
    servers.insert(
        "upstash/context7".to_string(),
        McpServer {
            server_type: None,
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "@upstash/context7".to_string()]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = CodexMcpConfig::from_manifest(&manifest).unwrap();
    let toml = config.to_config_string().unwrap();

    assert!(
        toml.contains("[mcp_servers.\"upstash/context7\"]"),
        "quoted key missing: {toml}"
    );
}

// ─── OpenCode (JSON mcp.*) — parity ────────────────────────────────────

#[test]
fn test_opencode_local_server_command_array() {
    // Oracle: ConvertTo-OpenCodeMcpConfig for local (stdio) — command+args merged
    let mut servers = HashMap::new();
    servers.insert(
        "memory".to_string(),
        McpServer {
            server_type: Some("stdio".to_string()),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "@mcp/server-memory".to_string()]),
            env: None,
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = OpenCodeMcpConfig::from_manifest(&manifest).unwrap();
    let json: Value = serde_json::from_str(&config.to_config_string().unwrap()).unwrap();

    let entry = &json["mcp"]["memory"];
    assert_eq!(entry["type"], "local");
    assert_eq!(
        entry["command"],
        serde_json::json!(["npx", "-y", "@mcp/server-memory"])
    );
    assert_eq!(entry["enabled"], true);
}

#[test]
fn test_opencode_remote_server_uses_url() {
    // Oracle: ConvertTo-OpenCodeMcpConfig for http type → type=remote, url
    let mut servers = HashMap::new();
    servers.insert(
        "remote".to_string(),
        McpServer {
            server_type: Some("http".to_string()),
            command: None,
            args: None,
            env: None,
            url: Some("https://api.example.com/mcp/".to_string()),
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = OpenCodeMcpConfig::from_manifest(&manifest).unwrap();
    let json: Value = serde_json::from_str(&config.to_config_string().unwrap()).unwrap();

    let entry = &json["mcp"]["remote"];
    assert_eq!(entry["type"], "remote");
    assert_eq!(entry["url"], "https://api.example.com/mcp/");
    assert_eq!(entry["enabled"], true);
    assert!(entry.get("command").is_none());
}

#[test]
fn test_opencode_env_vars_as_environment_key() {
    // Oracle: ConvertTo-OpenCodeMcpConfig uses `environment` key (not `env`)
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "token123".to_string());
    let mut servers = HashMap::new();
    servers.insert(
        "tool".to_string(),
        McpServer {
            server_type: None,
            command: Some("my-tool".to_string()),
            args: None,
            env: Some(env),
            url: None,
            headers: None,
        },
    );

    let manifest = McpManifest {
        servers,
        inputs: None,
    };
    let config = OpenCodeMcpConfig::from_manifest(&manifest).unwrap();
    let json: Value = serde_json::from_str(&config.to_config_string().unwrap()).unwrap();

    let entry = &json["mcp"]["tool"];
    assert_eq!(entry["environment"]["API_KEY"], "token123");
    // Must not use "env" key (that's Copilot's key name)
    assert!(entry.get("env").is_none());
}

#[test]
fn test_opencode_output_has_mcp_top_level_key() {
    // Oracle: Update-OpenCodeMcpConfig wraps entries under "mcp" key
    let manifest = McpManifest {
        servers: HashMap::new(),
        inputs: None,
    };
    let config = OpenCodeMcpConfig::from_manifest(&manifest).unwrap();
    let json: Value = serde_json::from_str(&config.to_config_string().unwrap()).unwrap();

    assert!(
        json.get("mcp").is_some(),
        "top-level 'mcp' key must be present"
    );
    assert!(json["mcp"].is_object());
}
