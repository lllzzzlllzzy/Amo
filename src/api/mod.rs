use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{
    middleware::{admin_auth::admin_auth_middleware, card_auth::card_auth_middleware},
    state::AppState,
};

pub mod admin;
pub mod analysis;
pub mod cards;
pub mod conflict;
pub mod emotional;

pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/cards/balance", get(cards::balance))
        .route("/analysis", post(analysis::submit))
        .route("/analysis/:task_id", get(analysis::poll))
        .route("/analysis/followup", post(analysis::followup))
        .route("/emotional/chat", post(emotional::chat))
        .route("/conflict/analyze", post(conflict::analyze))
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
