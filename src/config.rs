use std::{env, env::VarError};

pub struct Config {
    _github_private_key: String,
    _application_id: String,
    _namespace: String,
}

impl Config {
    pub fn new() -> Result<Config, VarError> {
        let github_private_key = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;
        let application_id = env::var("APPLICATION_ID")?;
        let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "kubesci".into());

        Ok(Config {
            _github_private_key: github_private_key,
            _application_id: application_id,
            _namespace: namespace,
        })
    }
}
