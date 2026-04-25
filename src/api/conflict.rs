use axum::{
    extract::State,
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;

use crate::prompts::{BASE_PERSONA, CONFLICT_ANALYSIS_SYSTEM};
use crate::{
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_CONFLICT: i64 = 10;
const COST_CONFLICT_FOLLOWUP: i64 = 5;

#[derive(Deserialize)]
pub struct ConflictRequest {
    pub description: String,
    pub background: Option<String>,
}

/// POST /conflict/analyze — 冲突分析（SSE）
pub async fn analyze(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<ConflictRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.description.chars().count() > 2000 {
        return Err(AppError::BadRequest("描述不能超过2000字".into()));
    }

    let content = match &req.background {
        Some(bg) => format!("=== 背景 ===\n{bg}\n\n=== 冲突经过 ===\n{}", req.description),
        None => req.description.clone(),
    };

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Smart,
        system: Some(CONFLICT_ANALYSIS_SYSTEM.to_string()),
        messages: vec![LlmMessage::user(content)],
        max_tokens: 2000,
    }, state.db.clone(), card.code.clone(), COST_CONFLICT))
}

#[derive(Deserialize)]
pub struct ConflictFollowupRequest {
    pub question: String,
    pub analysis: String,
    pub description: Option<String>,
    pub history: Option<Vec<LlmMessage>>,
}

/// POST /conflict/followup — 冲突分析追问（SSE）
pub async fn followup(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<ConflictFollowupRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.question.chars().count() > 1000 {
        return Err(AppError::BadRequest("追问内容不能超过1000字".into()));
    }
    if req.analysis.is_empty() {
        return Err(AppError::BadRequest("缺少 analysis 字段".into()));
    }

    let mut system = format!(
        "{BASE_PERSONA}\n\n你正在帮用户解读一份冲突分析，用户有追问。分析已包含完整信息，你有足够的信息直接回答。不要再追问用户，直接给出具体的解读或建议，保持客观中立。\n\n"
    );
    if let Some(desc) = &req.description {
        system.push_str(&format!("=== 原始冲突描述 ===\n{desc}\n\n"));
    }
    system.push_str(&format!("=== 冲突分析结果 ===\n{}", req.analysis));

    // 支持多轮追问：前端传入历史对话，追加当前问题
    let mut messages = req.history.unwrap_or_default();
    messages.push(LlmMessage::user(req.question));

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Smart,
        system: Some(system),
        messages,
        max_tokens: 1500,
    }, state.db.clone(), card.code.clone(), COST_CONFLICT_FOLLOWUP))
}
