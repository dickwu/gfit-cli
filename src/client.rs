//! Thin HTTP layer. Every GFIT endpoint is a POST that returns `{code, message, data}`.

use serde_json::Value;
use std::time::Duration;

pub struct Response {
    pub status: u16,
    pub body: Value,
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("gfit-cli/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

/// True for hosts we will follow self-update redirects to. GitHub release asset
/// downloads 302 from github.com to objects.githubusercontent.com; anything else
/// (a non-GitHub host, or plain http) is refused so a tampered redirect can't
/// point the updater at an arbitrary binary.
fn is_trusted_download_host(host: &str) -> bool {
    matches!(host, "github.com" | "githubusercontent.com")
        || host.ends_with(".github.com")
        || host.ends_with(".githubusercontent.com")
}

/// HTTP client for downloading release assets: short of following a redirect off
/// HTTPS or off a GitHub host, behaves like `build_client`.
fn build_download_client() -> Result<reqwest::blocking::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        let trusted = attempt.url().scheme() == "https"
            && attempt.url().host_str().map(is_trusted_download_host).unwrap_or(false);
        if !trusted {
            attempt.stop() // returns the 3xx as-is; caller rejects the non-2xx status
        } else if attempt.previous().len() >= 10 {
            attempt.error("too many redirects")
        } else {
            attempt.follow()
        }
    });
    reqwest::blocking::Client::builder()
        .user_agent(concat!("gfit-cli/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .redirect(policy)
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

fn finish(resp: reqwest::blocking::Response) -> Result<Response, String> {
    let status = resp.status().as_u16();
    let text = resp.text().map_err(|e| format!("failed to read response: {e}"))?;
    let body = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
    Ok(Response { status, body })
}

pub fn post_json(url: &str, body: &Value, token: Option<&str>) -> Result<Response, String> {
    let client = build_client()?;
    let mut req = client.post(url).json(body);
    if let Some(t) = token {
        req = req.header("Authorization", t);
    }
    let resp = req.send().map_err(|e| format!("request failed: {e}"))?;
    finish(resp)
}

pub fn post_multipart(
    url: &str,
    file: &str,
    type_val: &str,
    token: Option<&str>,
) -> Result<Response, String> {
    let form = reqwest::blocking::multipart::Form::new()
        .text("type", type_val.to_string())
        .file("image", file)
        .map_err(|e| format!("cannot read file '{file}': {e}"))?;
    let client = build_client()?;
    let mut req = client.post(url).multipart(form);
    if let Some(t) = token {
        req = req.header("Authorization", t);
    }
    let resp = req.send().map_err(|e| format!("request failed: {e}"))?;
    finish(resp)
}

/// GET a URL and parse the JSON body (used for the GitHub Releases API). Sends the
/// crate's User-Agent (GitHub requires one) and the GitHub JSON Accept header.
pub fn get_json(url: &str) -> Result<Value, String> {
    let client = build_client()?;
    let resp = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| format!("failed to read response: {e}"))?;
    if !status.is_success() {
        let detail = text.lines().next().unwrap_or("").to_string();
        return Err(format!("GitHub API HTTP {}: {detail}", status.as_u16()));
    }
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON from GitHub: {e}"))
}

/// GET a URL and return the raw bytes (release asset download). Redirects are
/// only followed to HTTPS GitHub hosts (see `build_download_client`).
pub fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = build_download_client()?;
    let resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "download failed: HTTP {} (refusing redirects off https://*.github.com)",
            resp.status().as_u16()
        ));
    }
    let bytes = resp.bytes().map_err(|e| format!("failed to read download: {e}"))?;
    Ok(bytes.to_vec())
}
