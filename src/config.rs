use std::{env, env::VarError};

#[derive(Clone)]
pub struct Config {
    pub github_private_key: String,
    pub application_id: String,
    pub namespace: String,
    pub github_base_url: String,
}

impl Config {
    pub fn new() -> Result<Config, VarError> {
        let github_private_key = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;
        let application_id = env::var("APPLICATION_ID")?;
        let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "kubesci".into());
        let github_base_url = "https://api.github.com".to_string();

        Ok(Config {
            github_private_key,
            application_id,
            namespace,
            github_base_url,
        })
    }
}
