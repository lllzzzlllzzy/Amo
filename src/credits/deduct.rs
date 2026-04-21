use sqlx::PgPool;
use crate::error::AppError;

pub async fn deduct_credits(
    db: &PgPool,
    code: &str,
    amount: i64,
) -> Result<i64, AppError> {
    let now = chrono::Utc::now().timestamp();

    // 单次查询：CTE 先尝试扣减，外层查询判断失败原因
    let row: Option<(Option<i64>, bool)> = sqlx::query_as(
        r#"
        WITH deducted AS (
            UPDATE cards SET credits = credits - $1
            WHERE code = $2 AND credits >= $1 AND (expires_at IS NULL OR expires_at > $3)
            RETURNING credits
        )
        SELECT
            (SELECT credits FROM deducted) AS remaining,
            EXISTS(SELECT 1 FROM cards WHERE code = $2) AS card_exists
        "#
    )
    .bind(amount)
    .bind(code)
    .bind(now)
    .fetch_optional(db)
    .await?;

    match row {
        Some((Some(remaining), _)) => Ok(remaining),
        Some((None, true)) => Err(AppError::InsufficientCredits),
        _ => Err(AppError::InvalidCard),
    }
}
