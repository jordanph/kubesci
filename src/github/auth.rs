use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::env;
use serde_derive::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: i64,
    iat: i64,
    iss: String
}

pub fn authenticate_app() -> Result<std::string::String, Box<dyn std::error::Error>> {
    let application_id = env::var("APPLICATION_ID")?;

    let now = Utc::now().timestamp();
    let ten_minutes_from_now = now + (10 * 60);

    let claim = Claims {
        exp: ten_minutes_from_now,
        iat: now,
        iss: application_id
    };

    let secret = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;

    let token = encode(&Header::new(Algorithm::RS256), &claim, &EncodingKey::from_rsa_pem(secret.as_bytes())?)?;

    return Ok(token);
}
