use axum::{
    extract::State,
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;

use crate::prompts::EMOTIONAL_SUPPORT_SYSTEM;
use crate::{
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_FIRST_TURN: i64 = 10;
const COST_FOLLOW_UP: i64 = 5;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub history: Option<Vec<LlmMessage>>,
}

/// POST /emotional/chat — 情绪疏导多轮对话（SSE）
pub async fn chat(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.message.chars().count() > 1000 {
        return Err(AppError::BadRequest("消息不能超过1000字".into()));
    }

    let history = req.history.unwrap_or_default();
    let cost = if history.is_empty() { COST_FIRST_TURN } else { COST_FOLLOW_UP };
    let mut messages = history;
    messages.push(LlmMessage::user(&req.message));

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Fast,
        system: Some(EMOTIONAL_SUPPORT_SYSTEM.to_string()),
        messages,
        max_tokens: 2000,
    }, state.db.clone(), card.code.clone(), cost))
}
