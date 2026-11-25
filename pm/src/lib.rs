//! Shared logic for Project Manager, used by the desktop app (src-tauri),
//! the `pm` command-line tool and its MCP server.

pub mod connect;
pub mod context;
pub mod library;
pub mod mcp;
pub mod project;
pub mod sessions;
pub mod templates;
