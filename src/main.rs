mod api;
mod analysis;
mod config;
mod credits;
mod error;
mod llm;
mod middleware;
mod prompts;
mod state;

use std::sync::Arc;
use dashmap::DashMap;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use llm::{anthropic::AnthropicClient, openai::OpenAiClient};
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "amo=debug,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&db).await?;

    let llm: Arc<dyn llm::LlmClient> = match config.llm_provider.as_str() {
        "openai" => {
            let key = config.openai_api_key.clone()
                .expect("OPENAI_API_KEY must be set when LLM_PROVIDER=openai");
            Arc::new(OpenAiClient::new(
                key,
                config.openai_base_url.clone(),
                config.openai_smart_model.clone(),
                config.openai_fast_model.clone(),
            ))
        }
        _ => Arc::new(AnthropicClient::new(
            config.anthropic_api_key.clone(),
            config.anthropic_base_url.clone(),
            config.anthropic_smart_model.clone(),
            config.anthropic_fast_model.clone(),
        )),
    };

    let state = AppState {
        db,
        llm,
        config: Arc::new(config.clone()),
        task_store: Arc::new(DashMap::new()),
    };

    let app = api::build_router(state.clone())
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // 后台定时清理过期任务（每 10 分钟清理超过 30 分钟的已完成/失败任务）
    let task_store = state.task_store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now().timestamp();
            let expired: Vec<String> = task_store.iter()
                .filter(|entry| {
                    let val = entry.value();
                    now - val.created_at > 1800 && !matches!(val.status, analysis::types::TaskStatus::Processing)
                })
                .map(|entry| entry.key().clone())
                .collect();
            if !expired.is_empty() {
                tracing::info!("清理 {} 个过期任务", expired.len());
                for key in expired {
                    task_store.remove(&key);
                }
            }
        }
    });

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Amo 后端启动，监听 {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
