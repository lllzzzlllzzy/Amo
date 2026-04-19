use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde_json::{json, Value};
use std::pin::Pin;
use tracing::debug;

use crate::error::AppError;
use crate::llm::{LlmClient, types::{LlmRequest, ModelTier, StreamChunk}};

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
    smart_model: String,
    fast_model: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, base_url: String, smart_model: String, fast_model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
            smart_model,
            fast_model,
        }
    }

    fn model_name(&self, tier: &ModelTier) -> &str {
        match tier {
            ModelTier::Smart => &self.smart_model,
            ModelTier::Fast => &self.fast_model,
        }
    }

    fn build_body(&self, req: &LlmRequest, stream: bool) -> Value {
        let messages: Vec<Value> = req.messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect();

        let mut body = json!({
            "model": self.model_name(&req.model),
            "max_tokens": req.max_tokens,
            "messages": messages,
            "stream": stream,
        });

        if let Some(system) = &req.system {
            body["system"] = json!(system);
        }

        body
    }

    fn request(&self, stream: bool) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}/v1/messages", self.base_url))
            // 同时发送两种认证头，兼容官方和第三方代理
            .header("x-api-key", &self.api_key)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn complete(&self, req: LlmRequest) -> Result<String, AppError> {
        let body = self.build_body(&req, false);
        debug!("Anthropic request: model={}", self.model_name(&req.model));

        let resp = self.request(false)
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

        let text = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| AppError::LlmError("响应格式异常".to_string()))?
            .to_string();

        Ok(text)
    }

    async fn stream(
        &self,
        req: LlmRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send>>, AppError> {
        let body = self.build_body(&req, true);

        let resp = self.request(true)
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
                    Err(e) => {
                        yield Err(AppError::LlmError(e.to_string()));
                        return;
                    }
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
                            if json["type"] == "content_block_delta" {
                                if let Some(text) = json["delta"]["text"].as_str() {
                                    yield Ok(StreamChunk::Delta(text.to_string()));
                                }
                            }
                            if json["type"] == "message_stop" {
                                yield Ok(StreamChunk::Done);
                                return;
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
