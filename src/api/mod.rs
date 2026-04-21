use std::sync::Arc;
use axum::{
    middleware,
    response::sse::{Event, Sse},
    routing::{get, post},
    Router,
};
use futures::Stream;
use std::convert::Infallible;

use crate::{
    llm::{LlmClient, types::{LlmRequest, StreamChunk}},
    middleware::{admin_auth::admin_auth_middleware, card_auth::card_auth_middleware},
    state::AppState,
};

pub mod admin;
pub mod analysis;
pub mod cards;
pub mod conflict;
pub mod emotional;

/// 公共 SSE 流式辅助：将 LlmRequest 转为标准 SSE 事件流
pub fn llm_sse_stream(
    llm: Arc<dyn LlmClient>,
    req: LlmRequest,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let stream = async_stream::stream! {
        match llm.stream(req).await {
            Err(e) => {
                yield Ok(Event::default().event("error").data(e.to_string()));
            }
            Ok(mut s) => {
                use futures::StreamExt;
                while let Some(chunk) = s.next().await {
                    match chunk {
                        Ok(StreamChunk::Delta(text)) => {
                            yield Ok(Event::default().data(
                                serde_json::to_string(&serde_json::json!({"delta": text})).unwrap()
                            ));
                        }
                        Ok(StreamChunk::Done) => {
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
    Sse::new(stream)
}

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/cards/balance", get(cards::balance))
        .route("/analysis", post(analysis::submit))
        .route("/analysis/:task_id", get(analysis::poll))
        .route("/analysis/followup", post(analysis::followup))
        .route("/emotional/chat", post(emotional::chat))
        .route("/conflict/analyze", post(conflict::analyze))
        .route("/conflict/followup", post(conflict::followup))
        .layer(middleware::from_fn_with_state(
            state.db.clone(),
            card_auth_middleware,
        ));

    let admin = Router::new()
        .route("/admin/cards", post(admin::generate_cards))
        .route("/admin/cards", get(admin::list_cards))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ));

    let public = Router::new()
        .route("/cards/verify", post(cards::verify));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(admin)
        .with_state(state)
}
