use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use crate::error::AppError;
use crate::llm::types::{LlmRequest, StreamChunk};

pub mod types;
pub mod anthropic;
pub mod openai;

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 非流式调用，返回完整响应文本
    async fn complete(&self, req: LlmRequest) -> Result<String, AppError>;

    /// 流式调用，返回 delta 流
    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send>>, AppError>;
}
