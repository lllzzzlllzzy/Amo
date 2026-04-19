use std::sync::Arc;
use sqlx::PgPool;
use dashmap::DashMap;
use crate::config::Config;
use crate::llm::LlmClient;
use crate::analysis::types::TaskStatus;

pub type TaskStore = Arc<DashMap<String, TaskStatus>>;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub llm: Arc<dyn LlmClient>,
    pub config: Arc<Config>,
    pub task_store: TaskStore,
}
