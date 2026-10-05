//! On-demand tool execution boundary.
//! Tools are invoked by the core when required; no permanent worker loop.

#[derive(Debug, Clone)]
pub struct ToolRequest {
    pub name: String,
    pub input: String,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub output: String,
}

pub trait Tool {
    fn name(&self) -> &str;
    fn execute(&self, request: &ToolRequest) -> Result<ToolResult, String>;
}
