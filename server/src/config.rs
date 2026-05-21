use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub base_url: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?,
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()?,
            base_url: env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string()),
            github_client_id: env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-jwt-secret-change-in-production".to_string()),
        })
    }

    pub fn github_callback_url(&self) -> String {
        format!("{}/auth/github/callback", self.base_url)
    }
}
