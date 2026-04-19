use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct CardContext {
    pub code: String,
    pub credits: i64,
}

pub async fn card_auth_middleware(
    State(db): State<PgPool>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let code = extract_card_code(&req)?;
    let now = chrono::Utc::now().timestamp();

    let credits: Option<i64> = sqlx::query_scalar(
        "SELECT credits FROM cards WHERE code = $1 AND (expires_at IS NULL OR expires_at > $2)"
    )
    .bind(&code)
    .bind(now)
    .fetch_optional(&db)
    .await?;

    let credits = credits.ok_or(AppError::InvalidCard)?;

    req.extensions_mut().insert(CardContext { code, credits });
    Ok(next.run(req).await)
}

fn extract_card_code(req: &Request) -> Result<String, AppError> {
    let auth = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::InvalidCard)?;

    let code = auth
        .strip_prefix("Bearer ")
        .ok_or(AppError::InvalidCard)?
        .trim()
        .to_string();

    if code.is_empty() {
        return Err(AppError::InvalidCard);
    }
    Ok(code)
}
