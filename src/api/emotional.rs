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

    deduct_credits(&state.db, &card.code, COST_PER_TURN).await?;

    let mut messages = req.history.unwrap_or_default();
    messages.push(LlmMessage::user(&req.message));

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Fast,
        system: Some(EMOTIONAL_SUPPORT_SYSTEM.to_string()),
        messages,
        max_tokens: 1000,
    }))
}
