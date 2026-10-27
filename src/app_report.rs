//! Workout-app problem reports ("Report" button) for admins:
//! `admin.app-report.list`, `admin.app-report.show` and `admin.app-report.reply`.
//!
//! These two are `Special` commands because they do more than one plain POST:
//! `list` pages through `admin/app-report/list` (fixed 20 per page server-side,
//! newest first) and filters by `--since` client-side; `show` decodes the
//! `info` / `logs` JSON strings the detail endpoint returns and can download the
//! presigned screenshot/attachment links. Both have a human view and a JSON view
//! (`--json` pretty, `--raw` compact) meant for unattended jobs.
//!
//! Report timestamps come back as API-server local time (America/Winnipeg).

use crate::args::ParsedArgs;
use crate::client;
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::America::Winnipeg;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub const STATUSES: &[&str] = &["pending", "dealing", "finished", "all"];
const LIST_PATH: &str = "admin/app-report/list";
const DETAIL_PATH: &str = "admin/app-report/detail";
const REPLY_PATH: &str = "admin/app-report/reply";
/// The API's cap on one reply (AppReportThread::MAX_CONTENT).
pub const MAX_REPLY_CHARS: usize = 5000;
const DEFAULT_LIMIT: usize = 50;
const DEFAULT_TAIL: usize = 40;
/// Safety stop for paging: 200 pages x 20 = 4000 reports.
const MAX_PAGES: u64 = 200;
const EXCERPT_CHARS: usize = 60;

pub struct Ctx {
    pub base: String,
    pub token: String,
}

impl Ctx {
    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base.trim_end_matches('/'), path)
    }

    /// POST and unwrap the `{code, msg, data}` envelope: success only when HTTP
    /// is 2xx AND `code == 1`; otherwise the API's own message is the error.
    fn call(&self, path: &str, body: Value) -> Result<Value, String> {
        let resp = client::post_json(&self.url(path), &body, Some(&self.token))?;
        let code = resp.body.get("code").and_then(Value::as_i64);
        if !(200..300).contains(&resp.status) || code != Some(1) {
            return Err(format!(
                "{path}: HTTP {} code {}: {}",
                resp.status,
                code.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
                api_message(&resp.body)
            ));
        }
        Ok(resp.body.get("data").cloned().unwrap_or(Value::Null))
    }
}

fn api_message(body: &Value) -> String {
    for k in ["msg", "message"] {
        if let Some(s) = body.get(k).and_then(Value::as_str) {
            return s.to_string();
        }
    }
    let s = body.to_string();
    s.chars().take(200).collect()
}

/// Current wall-clock time in America/Winnipeg (the API's timezone).
fn now_local() -> NaiveDateTime {
    Utc::now().with_timezone(&Winnipeg).naive_local()
}

/// Short zone label for "now", e.g. CDT / CST.
pub fn zone_label() -> String {
    Utc::now().with_timezone(&Winnipeg).format("%Z").to_string()
}

/// `--since`: a duration back from now (`30m`, `24h`, `7d`, `2w`) or an absolute
/// Winnipeg-local date/time (`2026-10-26`, `2026-10-26 09:00`, `2026-10-26T09:00:00`).
pub fn parse_since(raw: &str) -> Result<NaiveDateTime, String> {
    parse_since_at(raw, Utc::now())
}

pub fn parse_since_at(raw: &str, now: chrono::DateTime<Utc>) -> Result<NaiveDateTime, String> {
    let s = raw.trim();
    if let Some(unit) = s.chars().last() {
        let num = &s[..s.len() - unit.len_utf8()];
        if let (true, Ok(n)) = (unit.is_ascii_alphabetic(), num.parse::<i64>()) {
            let d = match unit {
                'm' => Duration::minutes(n),
                'h' => Duration::hours(n),
                'd' => Duration::days(n),
                'w' => Duration::weeks(n),
                _ => return Err(format!("--since: unknown unit '{unit}' (use m, h, d or w)")),
            };
            return Ok((now - d).with_timezone(&Winnipeg).naive_local());
        }
    }
    for f in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
    ] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Ok(dt);
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).expect("midnight is valid"));
    }
    Err(format!(
        "--since: expected a duration like 24h / 7d or a date like 2026-10-26, got '{raw}'"
    ))
}

fn created_at(row: &Value) -> Option<NaiveDateTime> {
    let s = row.get("created_at").and_then(Value::as_str)?;
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()
}

fn fmt_dt(dt: &NaiveDateTime) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// One-line, length-capped version of a free-text comment.
pub fn excerpt(text: &str, max: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        flat
    } else {
        let mut s: String = flat.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

fn pad(s: &str, width: usize) -> String {
    let s = excerpt(s, width);
    format!("{s:<width$}")
}

fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

fn int_of(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}

fn bool_of(v: &Value, k: &str) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(false)
}

fn json_mode(parsed: &ParsedArgs) -> Option<bool> {
    if parsed.has("raw") {
        Some(false)
    } else if parsed.has("json") {
        Some(true)
    } else {
        None
    }
}

fn print_json(v: &Value, pretty: bool) {
    let text = if pretty {
        serde_json::to_string_pretty(v)
    } else {
        serde_json::to_string(v)
    };
    println!("{}", text.unwrap_or_default());
}

pub struct ListOpts {
    pub status: String,
    pub since: Option<NaiveDateTime>,
    pub limit: usize,
}

pub fn list_opts(parsed: &ParsedArgs) -> Result<ListOpts, String> {
    let status = parsed.get("status").unwrap_or("pending").to_lowercase();
    if !STATUSES.contains(&status.as_str()) {
        return Err(format!(
            "--status: expected one of {}, got '{status}'",
            STATUSES.join("|")
        ));
    }
    let since = parsed.get("since").map(parse_since).transpose()?;
    let limit = match parsed.get("limit") {
        Some(v) => v
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("--limit: expected a positive integer, got '{v}'"))?,
        None => DEFAULT_LIMIT,
    };
    Ok(ListOpts {
        status,
        since,
        limit,
    })
}

pub fn list_body(status: &str, page: u64) -> Value {
    let mut body = json!({ "page": page });
    if status != "all" {
        body["status"] = json!(status);
    }
    body
}

pub fn dry_run_list(base: &str, parsed: &ParsedArgs) -> Result<(), String> {
    let o = list_opts(parsed)?;
    println!(
        "POST {}/{LIST_PATH}  (paged, 20 per page, newest first)",
        base.trim_end_matches('/')
    );
    println!("auth: Authorization: <token>");
    println!("body: {}", list_body(&o.status, 1));
    println!(
        "client-side: limit {}{}",
        o.limit,
        o.since
            .map(|s| format!(", created_at >= {} (America/Winnipeg)", fmt_dt(&s)))
            .unwrap_or_default()
    );
    Ok(())
}

pub fn list(ctx: &Ctx, parsed: &ParsedArgs) -> Result<(), String> {
    let o = list_opts(parsed)?;
    let mut reports: Vec<Value> = Vec::new();
    let mut counts = Value::Null;
    let mut total: u64 = 0;
    let mut has_more = false;
    let mut page: u64 = 1;

    'pages: loop {
        let data = ctx.call(LIST_PATH, list_body(&o.status, page))?;
        if page == 1 {
            counts = data.get("status_counts").cloned().unwrap_or(Value::Null);
            total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
        }
        let rows = data
            .get("list")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if rows.is_empty() {
            break;
        }
        for row in rows {
            // Newest first, so the first row older than --since ends the walk.
            if let (Some(cut), Some(at)) = (o.since, created_at(&row)) {
                if at < cut {
                    break 'pages;
                }
            }
            if reports.len() >= o.limit {
                has_more = true;
                break 'pages;
            }
            reports.push(row);
        }
        let page_size = data
            .get("page_size")
            .and_then(Value::as_u64)
            .unwrap_or(20)
            .max(1);
        if page * page_size >= total || page >= MAX_PAGES {
            break;
        }
        page += 1;
    }

    if let Some(pretty) = json_mode(parsed) {
        let out = json!({
            "generated_at": fmt_dt(&now_local()),
            "timezone": "America/Winnipeg",
            "filter": {
                "status": o.status,
                "since": o.since.map(|s| fmt_dt(&s)),
                "limit": o.limit,
            },
            "status_counts": counts,
            "count": reports.len(),
            "has_more": has_more,
            "reports": reports,
        });
        print_json(&out, pretty);
        return Ok(());
    }

    let tz = zone_label();
    let counts_line = ["pending", "dealing", "finished", "all"]
        .iter()
        .map(|k| {
            format!(
                "{k} {}",
                counts.get(*k).and_then(Value::as_i64).unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let since_txt = o
        .since
        .map(|s| format!(" since {}", s.format("%Y-%m-%d %H:%M")))
        .unwrap_or_default();
    println!(
        "App reports — {}{since_txt}: {} shown{}   [{counts_line}]",
        o.status,
        reports.len(),
        if has_more {
            " (more — raise --limit)"
        } else {
            ""
        }
    );
    if reports.is_empty() {
        return Ok(());
    }
    println!();
    println!(
        "{:>5}  {:<16}  {:<8}  {:<24}  {:<22}  {:<8}  {:>5}  COMMENT",
        "ID",
        format!("CREATED ({tz})"),
        "STATUS",
        "REPORTER (UID)",
        "PAGE",
        "VERSION",
        "FILES"
    );
    for r in &reports {
        let created = created_at(r)
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| str_of(r, "created_at").to_string());
        let reporter = format!("{} ({})", str_of(r, "user_name"), int_of(r, "user_id"));
        let files = int_of(r, "attachment_count") + i64::from(bool_of(r, "has_screenshot"));
        println!(
            "{:>5}  {:<16}  {:<8}  {}  {}  {}  {:>5}  {}",
            int_of(r, "id"),
            created,
            str_of(r, "status"),
            pad(&reporter, 24),
            pad(str_of(r, "tag"), 22),
            pad(str_of(r, "app_version"), 8),
            files,
            excerpt(str_of(r, "comment"), EXCERPT_CHARS)
        );
    }
    println!("\nDetails: gfit-cli admin.app-report.show --id <ID>");
    Ok(())
}

/// The report id from `--id N`, a bare positional `N`, or `--json N` (the
/// parser binds a following bare value to `--json`).
pub fn show_id(parsed: &ParsedArgs) -> Result<u64, String> {
    let candidate = parsed
        .get("id")
        .or_else(|| {
            parsed
                .items
                .iter()
                .find(|(k, v)| k.is_empty() && v.is_some())
                .and_then(|(_, v)| v.as_deref())
        })
        .or_else(|| parsed.get("json"))
        .or_else(|| parsed.get("raw"));
    match candidate {
        Some(v) => v
            .parse::<u64>()
            .map_err(|_| format!("report id must be a number, got '{v}'")),
        None => Err("missing report id: gfit-cli admin.app-report.show --id <ID>".into()),
    }
}

pub fn dry_run_show(base: &str, parsed: &ParsedArgs) -> Result<(), String> {
    let id = show_id(parsed)?;
    println!("POST {}/{DETAIL_PATH}", base.trim_end_matches('/'));
    println!("auth: Authorization: <token>");
    println!("body: {}", json!({ "id": id }));
    if let Some(dir) = parsed.get("download") {
        println!("then: download screenshot + attachments into {dir}");
    }
    Ok(())
}

/// Decode a JSON-in-a-string payload; anything unparsable is kept as the raw string.
fn decode(v: Option<&Value>) -> Value {
    match v {
        Some(Value::String(s)) if s.is_empty() => Value::Null,
        Some(Value::String(s)) => {
            serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
        }
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

/// `-m:ss` before the report was captured, matching the app's session-log view.
pub fn before_report(ts: i64, captured: i64) -> String {
    let secs = ((captured - ts).max(0) as f64 / 1000.0).round() as i64;
    format!("-{}:{:02}", secs / 60, secs % 60)
}

fn log_entries(logs: &Value) -> Vec<Value> {
    logs.get("entries")
        .and_then(Value::as_array)
        .or_else(|| logs.as_array())
        .cloned()
        .unwrap_or_default()
}

/// File extension of a presigned URL's object key (`.../abc.png?X-Amz-...` → png).
pub fn url_ext(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let last = path.rsplit('/').next()?;
    let (_, ext) = last.rsplit_once('.')?;
    (!ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .then(|| ext.to_lowercase())
}

pub fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('.').to_string();
    if s.is_empty() {
        "file".into()
    } else {
        s
    }
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let bytes = client::get_bytes(url)?;
    std::fs::write(dest, bytes).map_err(|e| format!("cannot write {}: {e}", dest.display()))
}

pub fn show(ctx: &Ctx, parsed: &ParsedArgs) -> Result<(), String> {
    let id = show_id(parsed)?;
    let tail = match parsed.get("tail") {
        Some(v) => v
            .parse::<usize>()
            .map_err(|_| format!("--tail: expected a number, got '{v}'"))?,
        None => DEFAULT_TAIL,
    };
    let data = ctx.call(DETAIL_PATH, json!({ "id": id }))?;
    let info = decode(data.get("info"));
    let logs = decode(data.get("logs"));
    let attachments = data
        .get("attachments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let screenshot = str_of(&data, "screenshot_url").to_string();

    let mut downloaded: Vec<String> = Vec::new();
    // A missing object (e.g. a screenshot purged from storage) is reported but
    // doesn't cost the rest of the report.
    let mut download_errors: Vec<String> = Vec::new();
    let mut fetch = |url: &str, dest: PathBuf| match download(url, &dest) {
        Ok(()) => downloaded.push(dest.display().to_string()),
        Err(e) => download_errors.push(format!("{}: {e}", dest.display())),
    };
    if let Some(dir) = parsed.get("download") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        if !screenshot.is_empty() {
            let ext = url_ext(&screenshot).unwrap_or_else(|| "png".into());
            let dest = dir.join(format!("report-{id}-screenshot.{ext}"));
            fetch(&screenshot, dest);
        }
        for (i, a) in attachments.iter().enumerate() {
            let url = str_of(a, "url");
            if url.is_empty() {
                continue;
            }
            let dest = dir.join(format!(
                "report-{id}-{}-{}",
                i + 1,
                safe_name(str_of(a, "name"))
            ));
            fetch(url, dest);
        }
    }

    if let Some(pretty) = json_mode(parsed) {
        let mut out: Map<String, Value> = data.as_object().cloned().unwrap_or_default();
        out.insert("info".into(), info);
        out.insert("logs".into(), logs);
        out.insert("downloaded".into(), json!(downloaded));
        out.insert("download_errors".into(), json!(download_errors));
        print_json(&Value::Object(out), pretty);
        return Ok(());
    }

    let tz = zone_label();
    println!(
        "App report #{} — {}",
        int_of(&data, "id"),
        str_of(&data, "status_label")
    );
    println!(
        "  Reporter:  {} (uid {})",
        str_of(&data, "user_name"),
        int_of(&data, "user_id")
    );
    println!("  Created:   {} {tz}", str_of(&data, "created_at"));
    println!(
        "  App:       {} {}",
        str_of(&data, "app"),
        str_of(&data, "app_version")
    );
    println!("  Page:      {}", str_of(&data, "tag"));
    if int_of(&data, "handled_by") != 0 {
        println!(
            "  Handled:   {} (uid {}) at {} {tz}",
            str_of(&data, "handled_by_name"),
            int_of(&data, "handled_by"),
            str_of(&data, "handled_at")
        );
    }

    println!("\nComment:");
    let comment = str_of(&data, "comment");
    if comment.trim().is_empty() {
        println!("  (empty)");
    }
    for line in comment.lines() {
        println!("  {line}");
    }

    println!("\nInfo:");
    match &info {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                println!("  {k}: {val}");
            }
        }
        Value::Null => println!("  (none)"),
        Value::Object(_) => println!("  (empty)"),
        other => println!("  {other}"),
    }

    let entries = log_entries(&logs);
    let captured = logs.get("captured_ts").and_then(Value::as_i64);
    let errors = entries
        .iter()
        .filter(|e| str_of(e, "level") == "error")
        .count();
    println!(
        "\nSession log: {} entries, {} errors{}",
        entries.len(),
        errors,
        if logs.get("truncated").and_then(Value::as_bool) == Some(true) {
            " (window truncated)"
        } else {
            ""
        }
    );
    if !entries.is_empty() && tail > 0 {
        let start = entries.len().saturating_sub(tail);
        if start > 0 {
            println!("  … {start} earlier entries hidden (use --tail N, or --json for all)");
        }
        for e in &entries[start..] {
            let when = match (e.get("ts").and_then(Value::as_i64), captured) {
                (Some(ts), Some(cap)) => before_report(ts, cap),
                _ => "-".into(),
            };
            println!(
                "  {:>6}  {:<5}  {:<7}  {}",
                when,
                str_of(e, "level"),
                str_of(e, "kind"),
                str_of(e, "msg")
            );
        }
    }

    print_thread(&data, &tz);

    println!("\nScreenshot:");
    if screenshot.is_empty() {
        println!("  (none)");
    } else {
        println!("  {screenshot}");
    }
    println!("\nAttachments: {}", attachments.len());
    for a in &attachments {
        println!(
            "  {} ({}, {} bytes)\n    {}",
            str_of(a, "name"),
            str_of(a, "type"),
            int_of(a, "size"),
            str_of(a, "url")
        );
    }
    if !screenshot.is_empty() || !attachments.is_empty() {
        println!("  (links are presigned and expire after 60 minutes)");
    }
    if !downloaded.is_empty() {
        println!("\nDownloaded:");
        for p in &downloaded {
            println!("  {p}");
        }
    }
    if !download_errors.is_empty() {
        println!("\nDownload failed:");
        for e in &download_errors {
            println!("  {e}");
        }
    }
    Ok(())
}

/// One conversation/timeline entry as a printable line block.
pub fn thread_line(e: &Value) -> String {
    let who = if bool_of(e, "from_admin") {
        format!("{} (GFIT)", str_of(e, "user_name"))
    } else {
        format!("{} (reporter)", str_of(e, "user_name"))
    };
    let when = str_of(e, "created_at");
    if str_of(e, "type") == "status_change" {
        let from = str_of(e, "from_status_label");
        let to = str_of(e, "to_status_label");
        return format!("  {when}  {who}: status {from} → {to}");
    }
    let seen = if bool_of(e, "read") { "seen" } else { "unread" };
    let mut out = format!("  {when}  {who} [{seen}]:");
    for line in str_of(e, "content").lines() {
        out.push_str("\n      ");
        out.push_str(line);
    }
    out
}

fn print_thread(data: &Value, tz: &str) {
    let thread = data
        .get("thread")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let unread = int_of(data, "unread_count");
    println!(
        "\nThread ({tz}): {} entries{}",
        thread.len(),
        if unread > 0 {
            format!(", {unread} unread from the reporter")
        } else {
            String::new()
        }
    );
    if thread.is_empty() {
        println!("  (no replies yet — gfit-cli admin.app-report.reply --id <ID> --message '...')");
    }
    for e in &thread {
        println!("{}", thread_line(e));
    }
}

/// `--message` validated the way the API does (trimmed, non-empty, capped).
pub fn reply_body(parsed: &ParsedArgs) -> Result<Value, String> {
    let id = show_id(parsed)?;
    let message = parsed
        .get("message")
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if message.is_empty() {
        return Err("--message is required and must not be blank".into());
    }
    let n = message.chars().count();
    if n > MAX_REPLY_CHARS {
        return Err(format!(
            "--message is {n} characters; the API accepts at most {MAX_REPLY_CHARS}"
        ));
    }
    Ok(json!({ "id": id, "content": message }))
}

pub fn dry_run_reply(base: &str, parsed: &ParsedArgs) -> Result<(), String> {
    let body = reply_body(parsed)?;
    println!("POST {}/{REPLY_PATH}", base.trim_end_matches('/'));
    println!("auth: Authorization: <token>");
    println!(
        "body: {}",
        serde_json::to_string_pretty(&body).unwrap_or_default()
    );
    Ok(())
}

pub fn reply(ctx: &Ctx, parsed: &ParsedArgs) -> Result<(), String> {
    let body = reply_body(parsed)?;
    let id = body["id"].as_u64().unwrap_or(0);
    let entry = ctx.call(REPLY_PATH, body)?;
    if let Some(pretty) = json_mode(parsed) {
        print_json(&entry, pretty);
        return Ok(());
    }
    println!(
        "Replied to report #{id} (thread entry {}) at {} {}",
        int_of(&entry, "id"),
        str_of(&entry, "created_at"),
        zone_label()
    );
    println!("Thread: gfit-cli admin.app-report.show --id {id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at() -> chrono::DateTime<Utc> {
        // 2026-10-27 16:00 UTC = 11:00 CDT
        Utc.with_ymd_and_hms(2026, 10, 27, 16, 0, 0).unwrap()
    }

    #[test]
    fn since_durations_are_winnipeg_local() {
        assert_eq!(
            fmt_dt(&parse_since_at("24h", at()).unwrap()),
            "2026-10-26 11:00:00"
        );
        assert_eq!(
            fmt_dt(&parse_since_at("30m", at()).unwrap()),
            "2026-10-27 10:30:00"
        );
        assert_eq!(
            fmt_dt(&parse_since_at("1w", at()).unwrap()),
            "2026-10-20 11:00:00"
        );
        assert!(parse_since_at("3y", at()).is_err());
    }

    #[test]
    fn since_absolute_dates() {
        assert_eq!(
            fmt_dt(&parse_since_at("2026-10-26", at()).unwrap()),
            "2026-10-26 00:00:00"
        );
        assert_eq!(
            fmt_dt(&parse_since_at("2026-10-26 09:15", at()).unwrap()),
            "2026-10-26 09:15:00"
        );
        assert!(parse_since_at("yesterday", at()).is_err());
    }

    #[test]
    fn excerpt_flattens_and_caps() {
        assert_eq!(excerpt("a\n b\tc", 10), "a b c");
        assert_eq!(excerpt("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn before_report_formats_minutes() {
        assert_eq!(before_report(1_000, 253_000), "-4:12");
        assert_eq!(before_report(5_000, 1_000), "-0:00");
    }

    #[test]
    fn url_ext_and_safe_name() {
        assert_eq!(
            url_ext("https://x/app-report/2026/10/ab.png?X-Amz-Sig=1").as_deref(),
            Some("png")
        );
        assert_eq!(url_ext("https://x/noext?y=1.2"), None);
        assert_eq!(safe_name("../my clip.mov"), "_my_clip.mov");
    }

    #[test]
    fn thread_lines() {
        let status = json!({
            "type": "status_change", "from_admin": true, "user_name": "Peilin",
            "from_status_label": "Pending", "to_status_label": "In progress",
            "created_at": "2026-10-27 12:00:00"
        });
        assert_eq!(
            thread_line(&status),
            "  2026-10-27 12:00:00  Peilin (GFIT): status Pending → In progress"
        );
        let msg = json!({
            "type": "message", "from_admin": false, "user_name": "Dani",
            "content": "hi\nthere", "read": false, "created_at": "2026-10-27 12:01:00"
        });
        assert_eq!(
            thread_line(&msg),
            "  2026-10-27 12:01:00  Dani (reporter) [unread]:\n      hi\n      there"
        );
    }

    #[test]
    fn list_body_omits_status_for_all() {
        assert_eq!(list_body("all", 2), json!({ "page": 2 }));
        assert_eq!(
            list_body("pending", 1),
            json!({ "page": 1, "status": "pending" })
        );
    }
}
