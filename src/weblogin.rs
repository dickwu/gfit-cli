//! Browser-based login. `auth.login` with no `--email`/`--password` runs this:
//! it binds a localhost-only HTTP server on a random port, opens the browser to
//! it, serves a small login page, and waits for the page to POST the credentials
//! back. The credentials are exchanged for a token via `auth/login` and saved —
//! so the password never lands in shell history or process arguments.
//!
//! The flow is fully self-contained: no external service, redirect, or backend
//! change is involved. The server binds only `127.0.0.1`, uses an unguessable
//! random port plus a per-run nonce that the page must echo, serves over HTTP
//! (localhost only), and shuts down the moment login succeeds or 5 minutes pass.

use crate::login;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long to wait for the user to finish the browser login before giving up.
const DEADLINE: Duration = Duration::from_secs(300);

/// Run the browser login flow against `base_url`, pre-filling the form with
/// `prefill_email` when provided. Returns once a token has been saved.
pub fn run(base_url: &str, prefill_email: Option<&str>) -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("could not start the local login server: {e}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("could not configure the local login server: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("could not read the local login server address: {e}"))?
        .port();

    let nonce = nonce(port);
    let url = format!("http://127.0.0.1:{port}/");
    let page = login_page(&nonce, prefill_email.unwrap_or(""));

    println!("Opening your browser to sign in to GFIT…");
    if !open_browser(&url) {
        println!("Could not open a browser automatically.");
    }
    println!("If it doesn't open, visit this URL on this computer:\n  {url}");
    println!("Waiting for you to sign in (press Ctrl-C to cancel)…");

    let deadline = Instant::now() + DEADLINE;
    loop {
        if Instant::now() >= deadline {
            return Err(
                "timed out waiting for browser sign-in (5 min). Re-run, or pass --email/--password."
                    .into(),
            );
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Outcome::Success = handle(stream, &nonce, &page, base_url) {
                    println!(
                        "Login successful. Token saved to {}",
                        crate::config::config_path().display()
                    );
                    return Ok(());
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("local login server error: {e}")),
        }
    }
}

enum Outcome {
    /// The request was served but login is not complete (page load, favicon, or a
    /// failed attempt the user can retry); keep listening.
    Pending,
    /// Credentials were accepted and the token saved; stop listening.
    Success,
}

fn handle(mut stream: TcpStream, nonce: &str, page: &str, base_url: &str) -> Outcome {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(20)));

    let req = match read_request(&mut stream) {
        Some(r) => r,
        None => return Outcome::Pending,
    };

    if req.method == "POST" && req.path == "/submit" {
        let v: serde_json::Value =
            serde_json::from_str(&req.body).unwrap_or(serde_json::Value::Null);
        // The page must echo our per-run nonce; a blind cross-site POST cannot read it.
        if v.get("nonce").and_then(|x| x.as_str()) != Some(nonce) {
            write_json(
                &mut stream,
                403,
                &json!({"ok": false, "message": "bad request token"}),
            );
            return Outcome::Pending;
        }
        let email = v.get("email").and_then(|x| x.as_str()).unwrap_or("").trim();
        let password = v.get("password").and_then(|x| x.as_str()).unwrap_or("");
        if email.is_empty() || password.is_empty() {
            write_json(
                &mut stream,
                200,
                &json!({"ok": false, "message": "Email and password are required."}),
            );
            return Outcome::Pending;
        }
        match login::exchange(base_url, email, password) {
            Ok(()) => {
                write_json(&mut stream, 200, &json!({"ok": true}));
                // Give the browser a moment to read the response before we exit.
                std::thread::sleep(Duration::from_millis(150));
                Outcome::Success
            }
            Err(e) => {
                write_json(&mut stream, 200, &json!({"ok": false, "message": e}));
                Outcome::Pending
            }
        }
    } else if req.method == "GET" && (req.path == "/" || req.path.starts_with("/?")) {
        write_html(&mut stream, 200, page);
        Outcome::Pending
    } else {
        // favicon.ico and anything else.
        write_html(&mut stream, 404, "not found");
        Outcome::Pending
    }
}

struct Req {
    method: String,
    path: String,
    body: String,
}

/// Parse a single HTTP/1.1 request: the request line, just enough headers to find
/// `Content-Length`, and the body. Generic over `Read` so it is unit-testable
/// without a real socket.
fn read_request<R: Read>(r: &mut R) -> Option<Req> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = r.read(&mut tmp).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return None; // header too large; ignore this connection
        }
    };

    let head = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split_whitespace();
    let method = request_line.next()?.to_string();
    let path = request_line.next()?.to_string();

    let mut content_length = 0usize;
    for line in lines {
        if let Some((k, val)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        match r.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => body.extend_from_slice(&tmp[..n]),
            Err(_) => break,
        }
        if body.len() > 1024 * 1024 {
            break; // cap body at 1 MiB
        }
    }
    body.truncate(content_length.min(body.len()));
    Some(Req {
        method,
        path,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn write_html<W: Write>(w: &mut W, status: u16, body: &str) {
    write_response(w, status, "text/html; charset=utf-8", body.as_bytes());
}

fn write_json<W: Write>(w: &mut W, status: u16, body: &serde_json::Value) {
    write_response(w, status, "application/json", body.to_string().as_bytes());
}

fn write_response<W: Write>(w: &mut W, status: u16, content_type: &str, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = w.write_all(header.as_bytes());
    let _ = w.write_all(body);
    let _ = w.flush();
}

/// Unguessable per-run token derived (via SHA-256) from the wall clock, this
/// process id, and the bound port. sha2 is already a dependency; no `rand` crate
/// is pulled in just for this. Good enough for a localhost form that lives for at
/// most a few minutes.
fn nonce(port: u16) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    h.update(port.to_le_bytes());
    h.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Open `url` in the platform's default browser. Returns whether the launcher
/// reported success (the caller prints the URL regardless, as a fallback).
fn open_browser(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let status: std::io::Result<std::process::ExitStatus> = Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "unsupported platform",
    ));

    matches!(status, Ok(s) if s.success())
}

/// Escape a string for safe interpolation into an HTML attribute value.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render the login page, substituting the nonce and pre-filled email. Uses
/// `{{NONCE}}`/`{{EMAIL}}` placeholders (not `format!`) so the CSS/JS braces need
/// no escaping.
fn login_page(nonce: &str, prefill_email: &str) -> String {
    PAGE_TEMPLATE
        .replace("{{NONCE}}", nonce)
        .replace("{{EMAIL}}", &html_escape(prefill_email))
}

const PAGE_TEMPLATE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Sign in · gfit-cli</title>
<style>
  :root { color-scheme: light dark; }
  * { box-sizing: border-box; }
  body { margin:0; min-height:100vh; display:grid; place-items:center;
         font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif;
         background:#0e1116; color:#e6edf3; }
  .card { width:min(92vw,380px); background:#161b22; border:1px solid #30363d;
          border-radius:14px; padding:28px 26px; box-shadow:0 10px 30px rgba(0,0,0,.45); }
  h1 { margin:0 0 4px; font-size:20px; }
  p.sub { margin:0 0 18px; color:#8b949e; font-size:13px; }
  label { display:block; font-size:13px; margin:14px 0 6px; color:#c9d1d9; }
  input { width:100%; padding:10px 12px; border-radius:8px; border:1px solid #30363d;
          background:#0d1117; color:#e6edf3; font-size:14px; }
  input:focus { outline:none; border-color:#2f81f7; box-shadow:0 0 0 3px rgba(47,129,247,.3); }
  button { width:100%; margin-top:20px; padding:11px; border:0; border-radius:8px;
           background:#238636; color:#fff; font-size:14px; font-weight:600; cursor:pointer; }
  button:disabled { opacity:.6; cursor:default; }
  .msg { margin-top:14px; font-size:13px; min-height:18px; color:#ff7b72; }
  .done { text-align:center; }
  .done .check { font-size:44px; line-height:1; margin-bottom:8px; color:#3fb950; }
</style>
</head>
<body>
  <div class="card" id="card">
    <h1>Sign in to GFIT</h1>
    <p class="sub">Authorizing <b>gfit-cli</b> on this computer.</p>
    <form id="f">
      <label for="email">Email</label>
      <input id="email" type="email" autocomplete="username" value="{{EMAIL}}" required autofocus>
      <label for="password">Password</label>
      <input id="password" type="password" autocomplete="current-password" required>
      <button id="btn" type="submit">Sign in</button>
      <div class="msg" id="msg"></div>
    </form>
  </div>
<script>
  const NONCE = "{{NONCE}}";
  const f = document.getElementById('f');
  const btn = document.getElementById('btn');
  const msg = document.getElementById('msg');
  f.addEventListener('submit', async (e) => {
    e.preventDefault();
    msg.textContent = '';
    btn.disabled = true; btn.textContent = 'Signing in…';
    try {
      const r = await fetch('/submit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          email: document.getElementById('email').value,
          password: document.getElementById('password').value,
          nonce: NONCE,
        }),
      });
      const data = await r.json();
      if (data.ok) {
        document.getElementById('card').innerHTML =
          '<div class="done"><div class="check">&#10003;</div>' +
          '<h1>You&#39;re signed in</h1>' +
          '<p class="sub">Return to your terminal — you can close this tab.</p></div>';
      } else {
        msg.textContent = data.message || 'Sign-in failed.';
        btn.disabled = false; btn.textContent = 'Sign in';
      }
    } catch (err) {
      msg.textContent = 'Could not reach gfit-cli. Is it still running in your terminal?';
      btn.disabled = false; btn.textContent = 'Sign in';
    }
  });
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_locates_subslice() {
        assert_eq!(find(b"abc\r\n\r\nx", b"\r\n\r\n"), Some(3));
        assert_eq!(find(b"no header end", b"\r\n\r\n"), None);
    }

    #[test]
    fn read_request_parses_post_with_body() {
        let raw = "POST /submit HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 13\r\n\r\n{\"a\":\"bcdef\"}";
        let mut cursor = raw.as_bytes();
        let req = read_request(&mut cursor).expect("should parse");
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/submit");
        assert_eq!(req.body, "{\"a\":\"bcdef\"}");
    }

    #[test]
    fn read_request_parses_get_without_body() {
        let raw = "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let mut cursor = raw.as_bytes();
        let req = read_request(&mut cursor).expect("should parse");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/");
        assert!(req.body.is_empty());
    }

    #[test]
    fn nonce_is_32_hex_chars() {
        let n = nonce(12345);
        assert_eq!(n.len(), 32);
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn login_page_embeds_nonce_and_escaped_email() {
        let page = login_page("DEADBEEF", "a<b>&\"@x.com");
        assert!(page.contains("Sign in to GFIT"));
        assert!(page.contains("id=\"email\""));
        assert!(page.contains("id=\"password\""));
        assert!(page.contains("const NONCE = \"DEADBEEF\""));
        // Email is HTML-escaped inside the value attribute.
        assert!(page.contains("value=\"a&lt;b&gt;&amp;&quot;@x.com\""));
        assert!(!page.contains("{{NONCE}}"));
        assert!(!page.contains("{{EMAIL}}"));
    }

    #[test]
    fn write_response_has_status_and_content_length() {
        let mut out: Vec<u8> = Vec::new();
        write_response(&mut out, 200, "application/json", b"{\"ok\":true}");
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Type: application/json\r\n"));
        assert!(s.contains("Content-Length: 11\r\n"));
        assert!(s.ends_with("{\"ok\":true}"));
    }
}
