use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims<'a> {
    exp: i64,
    iat: i64,
    iss: &'a str,
}

pub fn authenticate_app(
    github_private_key: &str,
    application_id: &str,
    now: i64,
) -> Result<std::string::String, Box<dyn std::error::Error>> {
    let ten_minutes_from_now = now + (10 * 60);

    let claim = Claims {
        exp: ten_minutes_from_now,
        iat: now,
        iss: application_id,
    };

    let token = encode(
        &Header::new(Algorithm::RS256),
        &claim,
        &EncodingKey::from_rsa_pem(github_private_key.as_bytes())?,
    )?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_create_token() {
        let secret = "-----BEGIN RSA PRIVATE KEY-----
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
-----END RSA PRIVATE KEY-----";

        let application_id = "123456";

        let now = 0;

        let token = authenticate_app(secret, application_id, now).unwrap();

        let expected_token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJSUzI1NiJ9.eyJleHAiOjYwMCwiaWF0IjowLCJpc3MiOiIxMjM0NTYifQ.GXeWRVQ2TQ8SMJSP0Uq3nKHUt1fISkRzSYlCHNqZF_RSnfpHoXUmSIVLJRSrehN4hXtoQPl_LxDpD9Ag-dRzSBvcOer9iOQQM35ZyBnuhetYDbLDgUt7gGldnFEbT3TP0uQ5pL6JdSFRt-YkS58aRTi0dJXGePmVE6G_cjL9-Z_EDq2bbtiVkoWV_YKY92CW2ts3bLNnGYCqN_w8UIAPEpJEd2ZUrKUE9-AvaRbHJVo5MzTsLyoCK4vEid6KqP6T9URGSRNcrjEbAFmwSq2r8OYqDq31zsJKkZFSfVCMtEN1lmirv5IVrC3TVUUos2-Lv_C71H3KSGnsuyyHFghsS7AUPwfTL6t5Y3gdb-5hboipH29z0eb7TxrMTBecjpp3pEBLSy8A1G-t_WvqXLv-JY84ueFnCsz9uGvJmCE-yG44_kKJZMgClxio7-OdipXK8MJf2kNAMqBQ2Sr6bHMHhJCJCRhOlLUZwU0rV4SApGgKlei02sdLG3O2_pwDNAGdYHImUIHP0G7mnuxpuqGm37Z2v4__vTUTa-hB7lDJ3zhjisP7wWqFSTGDl_vnLuPYErI2g1tDFQegD9qW8DIjpSTwy48m8wmSZVnx5LvHtVdvQiz8YMWWLOMy6YdyaYV956ptHZGkwtOvTuNxgQ1ETKvLoIg0NiIPcGGdiFq3ATM";

        assert_eq!(token, expected_token)
    }
}
