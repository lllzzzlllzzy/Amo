use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("卡密无效或已过期")]
    InvalidCard,

    #[error("额度不足，请购买新的卡密")]
    InsufficientCredits,

    #[error("请求参数错误: {0}")]
    BadRequest(String),

    #[error("LLM 调用失败: {0}")]
    LlmError(String),

    #[error("数据库错误: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("内部错误")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::InvalidCard => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::InsufficientCredits => (StatusCode::PAYMENT_REQUIRED, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::LlmError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            AppError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
