pub mod agent_loop;
pub mod client;
pub mod registry;
pub mod web_fetch;

pub use agent_loop::{
    execute_tool_calls_parallel, run_agent_loop, AgentLoopConfig, StreamToolCallAccumulator,
};
pub use client::{
    create_mcp_client, HttpMcpClient, JsonRpcRequest, JsonRpcResponse, McpClient, StdioMcpClient,
};
pub use registry::{is_tool_allowed, McpToolRegistry};
pub use web_fetch::{execute_web_fetch, web_fetch_tool_def, WebFetchArgs, WEB_FETCH_TOOL_NAME};
