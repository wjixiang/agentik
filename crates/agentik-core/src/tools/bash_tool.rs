use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::tools::{ToolError, ToolFunction};

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(name = "bash", description = "Run shell command")]
pub struct BashInput {
    #[desc = "The command to execute"]
    pub command: String,
    pub timeout: usize,
    pub description: Option<String>,
}

pub struct BashTool;

#[async_trait]
impl ToolFunction for BashTool {
    type Input = BashInput;

    // definition() 继承自 ToolFunction trait 默认实现
    // → Self::Input::definition() → proc macro 生成的 ToolBuilder 链

    async fn run(&self, _input: Self::Input) -> Result<agentik_sdk::types::ToolResult, ToolError> {
        todo!()
    }
}
