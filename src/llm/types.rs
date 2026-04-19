use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".to_string(), content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".to_string(), content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: "assistant".to_string(), content: content.into() }
    }
}

#[derive(Debug, Clone)]
pub enum ModelTier {
    /// 高质量模型，用于分析任务
    Smart,
    /// 快速低成本模型，用于简单对话
    Fast,
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub model: ModelTier,
    pub system: Option<String>,
    pub messages: Vec<LlmMessage>,
    pub max_tokens: u32,
}

#[derive(Debug)]
pub enum StreamChunk {
    Delta(String),
    Done,
}
