use sqlx::PgPool;
use crate::error::AppError;

pub async fn deduct_credits(
    db: &PgPool,
    code: &str,
    amount: i64,
) -> Result<i64, AppError> {
    let now = chrono::Utc::now().timestamp();

    let result = sqlx::query_scalar::<_, i64>(
        "UPDATE cards SET credits = credits - $1 WHERE code = $2 AND credits >= $1 AND (expires_at IS NULL OR expires_at > $3) RETURNING credits"
    )
    .bind(amount)
    .bind(code)
    .bind(now)
    .fetch_optional(db)
    .await?;

    match result {
        Some(remaining) => Ok(remaining),
        None => {
            let credits: Option<i64> = sqlx::query_scalar(
                "SELECT credits FROM cards WHERE code = $1"
            )
            .bind(code)
            .fetch_optional(db)
            .await?;

            match credits {
                Some(c) if c < amount => Err(AppError::InsufficientCredits),
                Some(_) => Err(AppError::InsufficientCredits),
                None => Err(AppError::InvalidCard),
            }
        }
    }
}
