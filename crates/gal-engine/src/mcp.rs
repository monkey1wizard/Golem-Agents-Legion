//! MCP config: re-exports from the `mcp` domain crate (T-011, R-02).
//!
//! All generation logic (McpVariableResolver, McpMerger, McpProjection, etc.)
//! now lives in `crates/mcp`. This module is a thin compatibility shim so
//! `gal_engine::mcp::*` paths keep resolving during the JIT decomposition.
//!
//! `base::mcp` types (McpError, McpManifest, McpServer, Result) are also
//! re-exported via `crates/mcp`.

pub use mcp::{
    check_plugin_root_complete, load_manifest, save_projection, McpError, McpManifest,
    McpMerger, McpProjection, McpProjectionMetadata, McpServer, McpUpdateError,
    McpVariableResolver, Result,
};
