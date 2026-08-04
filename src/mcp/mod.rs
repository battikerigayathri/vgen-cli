//! MCP server — in-process wrappers around ResMate library commands.

pub mod dispatch;
pub mod handler;
pub mod tools;

pub use handler::{handle_request, run_stdio_server};
pub use tools::list_tool_definitions;
