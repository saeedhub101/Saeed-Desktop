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

/// The Core owns the registry; tools are resolved and executed only when requested.
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool + Send + Sync>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register<T>(&mut self, tool: T)
    where
        T: Tool + Send + Sync + 'static,
    {
        self.tools.push(Box::new(tool));
    }

    pub fn execute(&self, request: &ToolRequest) -> Result<ToolResult, String> {
        let tool = self.tools.iter()
            .find(|tool| tool.name() == request.name)
            .ok_or_else(|| format!("Tool not found: {}", request.name))?;
        tool.execute(request)
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.iter().map(|tool| tool.name().to_string()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoTool;

    impl Tool for EchoTool {
        fn name(&self) -> &str { "echo" }

        fn execute(&self, request: &ToolRequest) -> Result<ToolResult, String> {
            Ok(ToolResult { output: request.input.clone() })
        }
    }

    #[test]
    fn tools_are_resolved_on_demand() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);
        let result = registry.execute(&ToolRequest {
            name: "echo".into(),
            input: "hello".into(),
        }).expect("tool result");
        assert_eq!(result.output, "hello");
        assert_eq!(registry.names(), vec!["echo"]);
    }
}
