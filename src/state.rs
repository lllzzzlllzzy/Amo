use std::sync::Arc;
use sqlx::PgPool;
use crate::config::Config;
use crate::llm::LlmClient;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub llm: Arc<dyn LlmClient>,
    pub config: Arc<Config>,
}
