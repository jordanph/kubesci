use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use std::env;
use serde_derive::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    exp: i64, // Required (validate_exp defaults to true in validation). Expiration time
    iat: i64, // Optional. Issued at
}

pub fn authenticate_app() -> Result<std::string::String, Box<dyn std::error::Error>> {

    let now = Utc::now().timestamp();
    let ten_minutes_from_now = now + (10 * 60);

    let claim = Claims {
        exp: ten_minutes_from_now,
        iat: now,
    };

    let secret = env::var("GITHUB_APPLICATION_PRIVATE_KEY")?;

    let token = encode(&Header::new(Algorithm::RS256), &claim, &EncodingKey::from_rsa_pem(secret.as_bytes())?)?;

    return Ok(token);
}
