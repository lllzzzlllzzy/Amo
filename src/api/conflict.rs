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

const COST_CONFLICT: i64 = 8;
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

    let system = format!(
        "{BASE_PERSONA}\n\n你正在帮用户解读一份冲突分析，用户有追问。基于之前的分析内容回答，保持客观中立。"
    );

    let mut context = String::new();
    if let Some(desc) = &req.description {
        context.push_str(&format!("=== 原始冲突描述 ===\n{desc}\n\n"));
    }
    context.push_str(&format!(
        "=== 冲突分析结果 ===\n{}\n\n=== 用户追问 ===\n{}",
        req.analysis, req.question
    ));

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Smart,
        system: Some(system),
        messages: vec![LlmMessage::user(context)],
        max_tokens: 1500,
    }, state.db.clone(), card.code.clone(), COST_CONFLICT_FOLLOWUP))
}
