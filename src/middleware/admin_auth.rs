use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use crate::{error::AppError, state::AppState};

pub async fn admin_auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get("X-Admin-Token")
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::InvalidCard)?;

    if token != state.config.admin_token {
        return Err(AppError::InvalidCard);
    }

    Ok(next.run(req).await)
}
