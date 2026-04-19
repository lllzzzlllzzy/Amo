use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use crate::{error::AppError, state::AppState};

#[derive(Deserialize)]
pub struct GenerateCardsRequest {
    /// 生成数量，最多 100 张
    pub count: u32,
    /// 每张卡密的初始额度
    pub credits: i64,
    /// 过期时间（Unix 时间戳），None 表示永不过期
    pub expires_at: Option<i64>,
}

/// POST /admin/cards — 批量生成卡密
pub async fn generate_cards(
    State(state): State<AppState>,
    Json(req): Json<GenerateCardsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req.count == 0 || req.count > 100 {
        return Err(AppError::BadRequest("count 必须在 1-100 之间".to_string()));
    }
    if req.credits <= 0 {
        return Err(AppError::BadRequest("credits 必须大于 0".to_string()));
    }

    let now = chrono::Utc::now().timestamp();
    let mut codes = Vec::with_capacity(req.count as usize);

    for _ in 0..req.count {
        let code = generate_code();
        sqlx::query(
            "INSERT INTO cards (code, credits, total, created_at, expires_at) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(&code)
        .bind(req.credits)
        .bind(req.credits)
        .bind(now)
        .bind(req.expires_at)
        .execute(&state.db)
        .await?;

        codes.push(code);
    }

    Ok(Json(json!({
        "count": codes.len(),
        "credits_per_card": req.credits,
        "expires_at": req.expires_at,
        "codes": codes,
    })))
}

/// GET /admin/cards — 查看所有卡密状态
pub async fn list_cards(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let rows = sqlx::query_as::<_, (String, i64, i64, i64, Option<i64>)>(
        "SELECT code, credits, total, created_at, expires_at FROM cards ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    let cards: Vec<_> = rows.into_iter().map(|(code, credits, total, created_at, expires_at)| {
        json!({
            "code": code,
            "credits": credits,
            "total": total,
            "used": total - credits,
            "created_at": created_at,
            "expires_at": expires_at,
        })
    }).collect();

    Ok(Json(json!({ "cards": cards, "total": cards.len() })))
}

/// 生成格式为 AMO-XXXX-XXXX-XXXX 的卡密
fn generate_code() -> String {
    let id = Uuid::new_v4().to_string().replace('-', "").to_uppercase();
    format!("AMO-{}-{}-{}", &id[0..4], &id[4..8], &id[8..12])
}
