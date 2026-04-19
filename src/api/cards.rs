use axum::{extract::State, Extension, Json};
use serde_json::json;
use crate::{error::AppError, middleware::card_auth::CardContext, state::AppState};

pub async fn verify(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let code = body["code"]
        .as_str()
        .ok_or_else(|| AppError::BadRequest("缺少 code 字段".to_string()))?;

    let now = chrono::Utc::now().timestamp();

    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT credits, total FROM cards WHERE code = $1 AND (expires_at IS NULL OR expires_at > $2)"
    )
    .bind(code)
    .bind(now)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some((credits, total)) => Ok(Json(json!({
            "valid": true,
            "credits": credits,
            "total": total,
        }))),
        None => Ok(Json(json!({ "valid": false }))),
    }
}

pub async fn balance(
    Extension(card): Extension<CardContext>,
) -> Json<serde_json::Value> {
    Json(json!({ "credits": card.credits }))
}
