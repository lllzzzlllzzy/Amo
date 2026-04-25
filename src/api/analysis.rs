use std::sync::Arc;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::{Stream, StreamExt};
use std::convert::Infallible;

use crate::prompts::BASE_PERSONA;
use crate::{
    analysis::{
        pipeline::{AnalysisPipeline, SectionEvent},
        types::AnalysisRequest,
    },
    credits::deduct::deduct_credits,
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_ANALYSIS: i64 = 20;
const COST_FOLLOWUP: i64 = 5;

/// POST /analysis — 提交分析请求，SSE 流式逐步返回各 section
pub async fn submit(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<AnalysisRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if req.messages.is_empty() {
        return Err(AppError::BadRequest("消息列表不能为空".to_string()));
    }
    if req.messages.len() > 100 {
        return Err(AppError::BadRequest("消息数量不能超过100条".to_string()));
    }
    for msg in &req.messages {
        if msg.text.chars().count() > 500 {
            return Err(AppError::BadRequest("单条消息不能超过500字".to_string()));
        }
    }

    let db = state.db.clone();
    let card_code = card.code.clone();
    let pipeline = Arc::new(AnalysisPipeline::new(state.llm.clone()));

    let stream = async_stream::stream! {
        let mut section_stream = std::pin::pin!(pipeline.run_streaming(req));
        let mut credited = false;

        while let Some(event) = section_stream.next().await {
            match &event {
                SectionEvent::Section { .. } => {
                    if !credited {
                        if let Err(e) = deduct_credits(&db, &card_code, COST_ANALYSIS).await {
                            yield Ok(Event::default().event("error").data(e.to_string()));
                            return;
                        }
                        credited = true;
                    }
                    let data = serde_json::to_string(&event).unwrap_or_default();
                    yield Ok(Event::default().data(data));
                }
                SectionEvent::Error { message } => {
                    yield Ok(Event::default().event("error").data(message.clone()));
                    return;
                }
            }
        }

        yield Ok(Event::default().event("done").data(""));
    };

    Ok(Sse::new(stream))
}

/// POST /analysis/followup — 针对报告追问（SSE 流式）
pub async fn followup(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(body): Json<serde_json::Value>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let question = body["question"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("缺少 question 字段".to_string()))?
        .to_string();

    let report_json = body["report"].clone();
    if report_json.is_null() {
        return Err(AppError::BadRequest("缺少 report 字段".to_string()));
    }

    let system = format!(
        "{BASE_PERSONA}\n\n你正在帮用户解读一份关系分析报告，用户有追问。\n\n报告已包含完整分析，你有足够的信息直接回答。不要再追问用户，直接给出具体的解读或建议。\n\n=== 分析报告 ===\n{}",
        serde_json::to_string_pretty(&report_json).unwrap_or_default()
    );

    let history: Vec<LlmMessage> = body["history"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| serde_json::from_value(m.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let mut messages = history;
    messages.push(LlmMessage::user(question));

    Ok(super::llm_sse_stream(state.llm.clone(), LlmRequest {
        model: ModelTier::Smart,
        system: Some(system),
        messages,
        max_tokens: 1500,
    }, state.db.clone(), card.code.clone(), COST_FOLLOWUP))
}
