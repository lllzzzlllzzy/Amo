use axum::{
    extract::State,
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::Stream;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;

use crate::prompts::CONFLICT_ANALYSIS_SYSTEM;
use crate::{
    credits::deduct::deduct_credits,
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_CONFLICT: i64 = 8;

#[derive(Deserialize)]
pub struct ConflictRequest {
    /// 用户描述的冲突经过
    pub description: String,
    /// 可选的背景信息
    pub background: Option<String>,
}

/// POST /conflict/analyze — 冲突分析（SSE）
pub async fn analyze(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<ConflictRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.description.chars().count() > 2000 {
        return Err(AppError::BadRequest("描述不能超过2000字".to_string()));
    }

    deduct_credits(&state.db, &card.code, COST_CONFLICT).await?;

    let content = match &req.background {
        Some(bg) => format!("=== 背景 ===\n{}\n\n=== 冲突经过 ===\n{}", bg, req.description),
        None => req.description.clone(),
    };

    let llm = state.llm.clone();
    let stream = async_stream::stream! {
        let llm_req = LlmRequest {
            model: ModelTier::Smart,
            system: Some(CONFLICT_ANALYSIS_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(content)],
            max_tokens: 2000,
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
