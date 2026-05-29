//! Shared login helpers used by both the direct (`--email`/`--password`) flow in
//! `main.rs` and the browser flow in `weblogin.rs`: turn a successful
//! `auth/login` response into a saved token, and (for the browser flow) exchange
//! raw credentials for that response.

use crate::{client, config};
use serde_json::{json, Value};

/// Persist the token from an `auth/login` response. On success the token (and the
/// email, when known) is written to the config file; on failure the API's
/// `message` is surfaced. The token-shape logic lives here so both login flows
/// stay in lockstep.
pub fn persist(resp: client::Response, email: Option<&str>) -> Result<(), String> {
    let code = resp.body.get("code").and_then(Value::as_i64);
    if code == Some(1) {
        let token = resp
            .body
            .get("data")
            .and_then(|d| d.get("token"))
            .and_then(Value::as_str)
            .ok_or("login succeeded but no token in response")?;
        let mut cfg = config::load();
        cfg.token = Some(token.to_string());
        if let Some(e) = email {
            cfg.email = Some(e.to_string());
        }
        config::save(&cfg).map_err(|e| format!("failed to save token: {e}"))?;
        Ok(())
    } else {
        let msg = resp
            .body
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("login failed");
        Err(format!("{msg} (code {:?}, HTTP {})", code, resp.status))
    }
}

/// POST `email` + `password` to `auth/login` and persist the returned token.
/// Used by the browser flow once the user submits the local web form.
pub fn exchange(base_url: &str, email: &str, password: &str) -> Result<(), String> {
    let url = format!("{}/auth/login", base_url.trim_end_matches('/'));
    let body = json!({ "email": email, "password": password });
    let resp = client::post_json(&url, &body, None)?;
    persist(resp, Some(email))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resp(status: u16, body: Value) -> client::Response {
        client::Response { status, body }
    }

    // One test owns the process-global GFIT_CONFIG to avoid racing parallel tests.
    #[test]
    fn persist_token_lifecycle() {
        let path =
            std::env::temp_dir().join(format!("gfit-login-test-{}.json", std::process::id()));
        std::env::set_var("GFIT_CONFIG", &path);
        let _ = std::fs::remove_file(&path);

        // Success: token + email are saved.
        persist(
            resp(200, json!({"code": 1, "data": {"token": "TOK123"}})),
            Some("me@example.com"),
        )
        .expect("persist should succeed");
        let saved = config::load();
        assert_eq!(saved.token.as_deref(), Some("TOK123"));
        assert_eq!(saved.email.as_deref(), Some("me@example.com"));

        // Failure (code != 1): surfaces the API message.
        let err = persist(resp(200, json!({"code": 0, "message": "bad creds"})), None).unwrap_err();
        assert!(err.contains("bad creds"), "got: {err}");

        // Success code but missing token: explicit error, no panic.
        let err = persist(resp(200, json!({"code": 1, "data": {}})), Some("x@y.z")).unwrap_err();
        assert!(err.contains("no token"), "got: {err}");

        let _ = std::fs::remove_file(&path);
        std::env::remove_var("GFIT_CONFIG");
    }
}
