//! TP-012: MCP provider serializer oracle parity tests
//!
//! Verifies that the Rust MCP provider serializers produce output that matches
//! the frozen PowerShell oracle script for Claude Desktop and Copilot CLI.
//!
//! Oracle scope (BUG-A): These are parity tests against the frozen Claude/Copilot
//! oracle. For bug-fix deltas (like dockeeper/doc-sync), use intent assertions
//! instead of oracle parity.

use gal_core::mcp::{McpManifest, McpServer};
use gal_core::providers::claude::ClaudeDesktopMcpConfig;
use gal_core::providers::copilot::CopilotCliMcpConfig;
use gal_core::providers::McpProviderConfig;
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn test_claude_desktop_simple_stdio_server() {
    // Oracle: ConvertTo-ClaudeDesktopServerEntry for a simple stdio server
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    // Verify structure matches oracle
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
    // Oracle: ConvertTo-ClaudeWrappedStdioCommand wraps env vars in PowerShell
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["test-server"];

    // Oracle wraps in powershell with env assignments
    assert_eq!(entry["command"], "powershell");
    assert!(entry["args"][0] == "-NoProfile");
    assert!(entry["args"][1] == "-Command");

    let script = entry["args"][2].as_str().unwrap();

    // Verify env assignments present
    assert!(script.contains("$env:API_KEY = 'secret123'"));
    assert!(script.contains("$env:PATH = '/usr/bin'"));

    // Verify command invocation
    assert!(script.contains("& 'node' 'index.js' '--verbose'"));
}

#[test]
fn test_claude_desktop_skip_http_server() {
    // Oracle: ConvertTo-ClaudeDesktopMcpServers skips servers with type=http
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    // Oracle excludes HTTP servers
    let servers_obj = value["mcpServers"].as_object().unwrap();
    assert_eq!(servers_obj.len(), 0);
}

#[test]
fn test_claude_desktop_skip_url_only_server() {
    // Oracle: ConvertTo-ClaudeDesktopMcpServers skips servers with url but no command
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let servers_obj = value["mcpServers"].as_object().unwrap();
    assert_eq!(servers_obj.len(), 0);
}

#[test]
fn test_claude_desktop_skip_unresolved_secret() {
    // Oracle: Test-McpServerHasUnresolvedSecrets filters out placeholder secrets
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    // Oracle excludes both servers with unresolved secrets
    let servers_obj = value["mcpServers"].as_object().unwrap();
    assert_eq!(servers_obj.len(), 0);
}

#[test]
fn test_copilot_cli_local_server() {
    // Oracle: ConvertTo-CopilotCliMcpConfig for local transport
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["test-server"];

    // Oracle: type=local, tools=["*"]
    assert_eq!(entry["type"], "local");
    assert_eq!(entry["tools"], serde_json::json!(["*"]));
    assert_eq!(entry["command"], "node");
    assert_eq!(entry["args"], serde_json::json!(["index.js"]));
}

#[test]
fn test_copilot_cli_local_with_env() {
    // Oracle: ConvertTo-CopilotCliMcpConfig preserves env for local transport
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["test-server"];

    assert_eq!(entry["type"], "local");
    assert!(entry.get("env").is_some());
    assert_eq!(entry["env"]["API_KEY"], "secret123");
}

#[test]
fn test_copilot_cli_http_server() {
    // Oracle: Get-CopilotCliTransport returns 'http' for http type
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
    let json = config.to_json_pretty().unwrap();
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
    // Oracle: Get-CopilotCliTransport returns 'sse' for sse type
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["sse-server"];

    assert_eq!(entry["type"], "sse");
    assert_eq!(entry["url"], "http://example.com/sse");
}

#[test]
fn test_copilot_cli_infer_transport_from_url() {
    // Oracle: Get-CopilotCliTransport infers 'http' when url present but no type
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["inferred"];
    assert_eq!(entry["type"], "http");
}

#[test]
fn test_copilot_cli_infer_transport_from_command() {
    // Oracle: Get-CopilotCliTransport infers 'local' when command present but no type
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let entry = &value["mcpServers"]["inferred"];
    assert_eq!(entry["type"], "local");
}

#[test]
fn test_copilot_cli_skip_unresolved_secret() {
    // Oracle: Test-McpServerHasUnresolvedSecrets also applies to Copilot CLI
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
    let json = config.to_json_pretty().unwrap();
    let value: Value = serde_json::from_str(&json).unwrap();

    let servers_obj = value["mcpServers"].as_object().unwrap();
    assert_eq!(servers_obj.len(), 0);
}

#[test]
fn test_claude_desktop_single_quote_escaping() {
    // Oracle: ConvertTo-PowerShellSingleQuotedLiteral escapes single quotes by doubling
    #[cfg(target_os = "windows")]
    {
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
        let json = config.to_json_pretty().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        let entry = &value["mcpServers"]["test"];
        let script = entry["args"][2].as_str().unwrap();

        // Oracle doubles single quotes for PowerShell escaping
        assert!(script.contains("$env:PATH = 'C:\\Program''s Files'"));
        assert!(script.contains("& 'node' '''quoted'''"));
    }
}
