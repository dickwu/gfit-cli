//! Thin HTTP layer. Every GFIT endpoint is a POST that returns `{code, message, data}`.

use serde_json::Value;

pub struct Response {
    pub status: u16,
    pub body: Value,
}

fn build_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("gfit-cli/", env!("CARGO_PKG_VERSION")))
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

/// GET a URL and return the raw bytes (release asset download; follows redirects).
pub fn download_bytes(url: &str) -> Result<Vec<u8>, String> {
    let client = build_client()?;
    let resp = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|e| format!("download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download failed: HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp.bytes().map_err(|e| format!("failed to read download: {e}"))?;
    Ok(bytes.to_vec())
}
