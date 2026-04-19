use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Extension, Json,
};
use futures::Stream;
use serde_json::json;
use std::convert::Infallible;
use uuid::Uuid;

use crate::prompts::BASE_PERSONA;
use crate::{
    analysis::{
        pipeline::AnalysisPipeline,
        types::{AnalysisRequest, TaskStatus},
    },
    credits::deduct::deduct_credits,
    error::AppError,
    llm::types::{LlmMessage, LlmRequest, ModelTier},
    middleware::card_auth::CardContext,
    state::AppState,
};

const COST_ANALYSIS: i64 = 20;
const COST_FOLLOWUP: i64 = 3;

/// POST /analysis — 提交分析请求，立即返回 task_id
pub async fn submit(
    State(state): State<AppState>,
    Extension(card): Extension<CardContext>,
    Json(req): Json<AnalysisRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 校验输入
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

    // 先扣额度
    deduct_credits(&state.db, &card.code, COST_ANALYSIS).await?;

    let task_id = Uuid::new_v4().to_string();
    let task_store = state.task_store.clone();
    let task_id_clone = task_id.clone();

    // 插入 processing 状态
    task_store.insert(task_id.clone(), TaskStatus::Processing);

    // 后台执行流水线
    let llm = state.llm.clone();
    tokio::spawn(async move {
        let pipeline = AnalysisPipeline::new(llm);
        let result = pipeline.run(&req).await;
        let status = match result {
            Ok(report) => TaskStatus::Done { report },
            Err(e) => TaskStatus::Failed { error: e.to_string() },
        };
        task_store.insert(task_id_clone, status);
    });

    Ok(Json(json!({
        "task_id": task_id,
        "status": "processing",
        "credits_used": COST_ANALYSIS,
    })))
}

/// GET /analysis/:task_id — 轮询分析状态
pub async fn poll(
    State(state): State<AppState>,
    Extension(_card): Extension<CardContext>,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let status = state.task_store.get(&task_id)
        .ok_or_else(|| AppError::BadRequest("任务不存在".to_string()))?;

    let resp = match status.value() {
        TaskStatus::Processing => json!({ "status": "processing" }),
        TaskStatus::Done { report } => json!({ "status": "done", "report": report }),
        TaskStatus::Failed { error } => json!({ "status": "failed", "error": error }),
    };

    Ok(Json(resp))
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

    // 先扣额度
    deduct_credits(&state.db, &card.code, COST_FOLLOWUP).await?;

    let system = format!(
        "{}\n\n你正在帮用户解读一份关系分析报告，用户有追问。",
        BASE_PERSONA
    );

    let context = format!(
        "=== 分析报告 ===\n{}\n\n=== 用户追问 ===\n{}",
        serde_json::to_string_pretty(&report_json).unwrap_or_default(),
        question
    );

    let llm = state.llm.clone();
    let stream = async_stream::stream! {
        let req = LlmRequest {
            model: ModelTier::Smart,
            system: Some(system),
            messages: vec![LlmMessage::user(context)],
            max_tokens: 1500,
        };

        match llm.stream(req).await {
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
