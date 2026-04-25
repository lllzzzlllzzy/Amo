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
    };

    let app = api::build_router(state)
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Amo 后端启动，监听 {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
