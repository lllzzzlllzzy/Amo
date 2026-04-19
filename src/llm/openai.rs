use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde_json::{json, Value};
use std::pin::Pin;

use crate::error::AppError;
use crate::llm::{LlmClient, types::{LlmRequest, ModelTier, StreamChunk}};

pub struct OpenAiClient {
    client: Client,
    api_key: String,
    base_url: String,
    smart_model: String,
    fast_model: String,
}

impl OpenAiClient {
    pub fn new(api_key: String, base_url: String, smart_model: String, fast_model: String) -> Self {
        Self { client: Client::new(), api_key, base_url, smart_model, fast_model }
    }

    fn model_name(&self, tier: &ModelTier) -> &str {
        match tier {
            ModelTier::Smart => &self.smart_model,
            ModelTier::Fast => &self.fast_model,
        }
    }

    fn build_messages(&self, req: &LlmRequest) -> Vec<Value> {
        let mut msgs = vec![];
        if let Some(system) = &req.system {
            msgs.push(json!({ "role": "system", "content": system }));
        }
        for m in &req.messages {
            msgs.push(json!({ "role": m.role, "content": m.content }));
        }
        msgs
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn complete(&self, req: LlmRequest) -> Result<String, AppError> {
        let body = json!({
            "model": self.model_name(&req.model),
            "max_tokens": req.max_tokens,
            "messages": self.build_messages(&req),
        });

        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmError(format!("HTTP {}: {}", status, text)));
        }

        let json: Value = resp.json().await
            .map_err(|e| AppError::LlmError(e.to_string()))?;

        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AppError::LlmError("响应格式异常".to_string()))?
            .to_string();

        Ok(text)
    }

    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send>>, AppError> {
        let body = json!({
            "model": self.model_name(&req.model),
            "max_tokens": req.max_tokens,
            "messages": self.build_messages(&req),
            "stream": true,
        });

        let resp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::LlmError(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::LlmError(format!("HTTP {}: {}", status, text)));
        }

        let stream = async_stream::stream! {
            use futures::StreamExt;
            let mut byte_stream = resp.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk) = byte_stream.next().await {
                let chunk = match chunk {
                    Ok(c) => c,
                    Err(e) => { yield Err(AppError::LlmError(e.to_string())); return; }
                };
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(pos) = buffer.find('\n') {
                    let line = buffer[..pos].trim().to_string();
                    buffer = buffer[pos + 1..].to_string();

                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            yield Ok(StreamChunk::Done);
                            return;
                        }
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(text) = json["choices"][0]["delta"]["content"].as_str() {
                                if !text.is_empty() {
                                    yield Ok(StreamChunk::Delta(text.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            yield Ok(StreamChunk::Done);
        };

        Ok(Box::pin(stream))
    }
}
