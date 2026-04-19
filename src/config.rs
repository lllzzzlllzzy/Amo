use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub llm_provider: String,

    // Anthropic（或兼容格式）
    pub anthropic_api_key: String,
    pub anthropic_base_url: String,
    pub anthropic_smart_model: String,
    pub anthropic_fast_model: String,

    pub openai_api_key: Option<String>,
    pub openai_base_url: String,
    pub openai_smart_model: String,
    pub openai_fast_model: String,

    pub host: String,
    pub port: u16,
    pub admin_token: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://amo.db".to_string()),
            llm_provider: env::var("LLM_PROVIDER")
                .unwrap_or_else(|_| "anthropic".to_string()),

            anthropic_api_key: env::var("ANTHROPIC_API_KEY")
                .or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"))
                .expect("ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN must be set"),
            anthropic_base_url: env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string()),
            anthropic_smart_model: env::var("ANTHROPIC_SMART_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),
            anthropic_fast_model: env::var("ANTHROPIC_FAST_MODEL")
                .unwrap_or_else(|_| "claude-haiku-4-5".to_string()),

            openai_api_key: env::var("OPENAI_API_KEY").ok(),
            openai_base_url: env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            openai_smart_model: env::var("OPENAI_SMART_MODEL")
                .unwrap_or_else(|_| "gpt-4o".to_string()),
            openai_fast_model: env::var("OPENAI_FAST_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),

            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .expect("PORT must be a number"),
            admin_token: env::var("ADMIN_TOKEN")
                .expect("ADMIN_TOKEN must be set"),
        }
    }
}
