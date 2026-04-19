use axum::{
    extract::State,
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::Stream;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;

use crate::prompts::EMOTIONAL_SUPPORT_SYSTEM;
use crate::{
    credits::deduct::deduct_credits,
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_PER_TURN: i64 = 2;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    /// 历史消息（role: user/assistant）
    pub history: Option<Vec<LlmMessage>>,
}

/// POST /emotional/chat — 情绪疏导多轮对话（SSE）
pub async fn chat(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.message.chars().count() > 1000 {
        return Err(AppError::BadRequest("消息不能超过1000字".to_string()));
    }

    deduct_credits(&state.db, &card.code, COST_PER_TURN).await?;

    let mut messages = req.history.unwrap_or_default();
    messages.push(LlmMessage::user(&req.message));

    let llm = state.llm.clone();
    let stream = async_stream::stream! {
        let llm_req = LlmRequest {
            model: ModelTier::Fast,
            system: Some(EMOTIONAL_SUPPORT_SYSTEM.to_string()),
            messages,
            max_tokens: 1000,
        };

        match llm.stream(llm_req).await {
            Err(e) => {
                yield Ok(Event::default().event("error").data(e.to_string()));
            }
            Ok(mut s) => {
                use futures::StreamExt;
                while let Some(chunk) = s.next().await {
                    match chunk {
                        Ok(crate::llm::types::StreamChunk::Delta(text)) => {
                            yield Ok(Event::default().data(
                                serde_json::to_string(&json!({"delta": text})).unwrap()
                            ));
                        }
                        Ok(crate::llm::types::StreamChunk::Done) => {
                            yield Ok(Event::default().event("done").data(""));
                            break;
                        }
                        Err(e) => {
                            yield Ok(Event::default().event("error").data(e.to_string()));
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(Sse::new(stream))
}
