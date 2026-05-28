//! gfit-cli — command-line client for the GFIT wellness API.
//!
//! Mirrors every API call the `workout` desktop app makes. Commands are
//! dot-named (`group.action`), e.g.:
//!
//!   gfit-cli auth.login --email me@x.com --password secret
//!   gfit-cli admin.members --type all --page 1
//!   gfit-cli coach.clients --search 'john'
//!   gfit-cli coach.weight.log-list --uid 42 --start_date 2026-01-01
//!
//! Every command supports `-h` / `--help` to print its API documentation.

mod args;
mod client;
mod config;
mod registry;
mod update;

use registry::{Command, Method, PType, Special};
use serde_json::{Map, Value};
use std::process::exit;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Flags consumed by the CLI itself rather than sent in the request body.
const RESERVED: &[&str] = &["json", "raw", "help", "h", "dry-run", "output", "o"];

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let reg = registry::registry();

    // No args, or a global help/version request.
    if argv.is_empty() {
        print_global_help(&reg);
        return;
    }
    match argv[0].as_str() {
        "-h" | "--help" | "help" => {
            print_global_help(&reg);
            return;
        }
        "-V" | "--version" | "version" => {
            println!("gfit-cli {VERSION}");
            return;
        }
        _ => {}
    }

    let name = &argv[0];
    let parsed = args::parse(&argv[1..]);

    let command = match reg.iter().find(|c| c.matches(name)) {
        Some(c) => c,
        None => {
            eprintln!("error: unknown command '{name}'");
            if let Some(sug) = suggest(&reg, name) {
                eprintln!("did you mean '{sug}'?");
            }
            eprintln!("\nrun `gfit-cli` to list all commands, or `gfit-cli <command> -h` for details.");
            exit(2);
        }
    };

    if parsed.help {
        print_command_help(command);
        return;
    }

    if let Err(e) = run(command, &parsed) {
        eprintln!("error: {e}");
        exit(1);
    }
}

fn run(command: &Command, parsed: &args::ParsedArgs) -> Result<(), String> {
    let cfg = config::load();

    // Local-only command: report saved auth.
    if command.special == Special::Status {
        let path = config::config_path();
        println!("config:  {}", path.display());
        println!("api url: {}", config::api_url(&cfg));
        match &cfg.email {
            Some(e) => println!("email:   {e}"),
            None => println!("email:   (not logged in)"),
        }
        println!("token:   {}", if cfg.token.is_some() { "present" } else { "absent" });
        return Ok(());
    }

    // Local-only command: self-update from GitHub Releases. Talks to GitHub, not the
    // GFIT API, so it runs without login and before the login gate below.
    if command.special == Special::Update {
        return update::run(parsed.has("check"), parsed.has("force"));
    }

    let dry = parsed.has("dry-run");

    // Login gate FIRST: every networked command requires a saved token EXCEPT
    // `auth.login` (local commands like `auth.status` returned earlier). Checked
    // before building/validating the body so a logged-out user always sees a clear
    // "not logged in" message for ANY command — never a confusing param error.
    // Under --dry-run we only inspect the request, so a missing token is tolerated.
    let needs_login = command.special != Special::Login;
    if needs_login && cfg.token.is_none() && !dry {
        return Err(
            "not logged in — run `gfit-cli auth.login --email <e> --password <p>` first".into(),
        );
    }

    let body = build_body(command, parsed)?;

    // Attach the token only to endpoints that take auth.
    let token: Option<String> = if command.auth { cfg.token.clone() } else { None };

    let base = config::api_url(&cfg);
    let url = format!("{}/{}", base.trim_end_matches('/'), command.path);

    // --dry-run: show what would be sent without sending it.
    if dry {
        let method = if command.method == Method::Multipart { "POST (multipart)" } else { "POST" };
        println!("{method} {url}");
        let auth_line = if !command.auth {
            "(none)"
        } else if token.is_some() {
            "Authorization: <token>"
        } else {
            "Authorization: <token>  (NOT logged in — this call would fail)"
        };
        println!("auth: {auth_line}");
        if command.method == Method::Multipart {
            println!("file: {:?}", body.get("file"));
            println!("type: {:?}", body.get("type"));
        } else {
            println!("body: {}", serde_json::to_string_pretty(&body).unwrap_or_default());
        }
        return Ok(());
    }

    let resp = match command.method {
        Method::Json => client::post_json(&url, &Value::Object(body.clone()), token.as_deref())?,
        Method::Multipart => {
            let file = body
                .get("file")
                .and_then(Value::as_str)
                .ok_or("missing --file <path>")?;
            let type_val = body.get("type").and_then(Value::as_str).unwrap_or("");
            client::post_multipart(&url, file, type_val, token.as_deref())?
        }
    };

    // Special: persist token on login.
    if command.special == Special::Login {
        return finish_login(resp, parsed, body);
    }
    // Special: clear token on logout regardless of server response shape.
    if command.special == Special::Logout {
        let mut cfg = config::load();
        cfg.token = None;
        let _ = config::save(&cfg);
    }

    let mut out = resp.body;
    // Special: client-side filter for coach.clients --search.
    if command.special == Special::ClientFilter {
        if let Some(needle) = parsed.get("search") {
            if !needle.is_empty() {
                filter_data(&mut out, needle);
            }
        }
    }

    print_response(&out, parsed, resp.status)
}

/// Build the JSON request body from defaults/constants, documented params, and
/// passthrough flags. `--json '{...}'` merges last.
fn build_body(command: &Command, parsed: &args::ParsedArgs) -> Result<Map<String, Value>, String> {
    let mut body = Map::new();

    for p in &command.params {
        let provided = parsed.get(&p.name);
        let raw: Option<String> = match provided {
            Some(v) => Some(v.to_string()),
            None => p.fixed.clone(),
        };
        match raw {
            Some(v) => {
                let val = p
                    .ty
                    .coerce(&v)
                    .map_err(|e| format!("--{}: {e}", p.name))?;
                body.insert(p.name.clone(), val);
            }
            None => {
                if p.required {
                    return Err(format!(
                        "missing required --{} <{}>  ({})",
                        p.name,
                        p.ty.label(),
                        p.desc
                    ));
                }
            }
        }
    }

    // Passthrough: any --key value not already handled and not reserved.
    let known: Vec<&str> = command.params.iter().map(|p| p.name.as_str()).collect();
    for (k, v) in &parsed.items {
        if k.is_empty() || known.contains(&k.as_str()) || RESERVED.contains(&k.as_str()) {
            continue;
        }
        if let Some(val) = v {
            body.insert(k.clone(), registry::coerce_loose(val));
        }
    }

    // --json merges a full object on top.
    if let Some(j) = parsed.get("json") {
        let extra: Value = serde_json::from_str(j).map_err(|e| format!("--json: invalid JSON: {e}"))?;
        if let Value::Object(m) = extra {
            for (k, v) in m {
                body.insert(k, v);
            }
        } else {
            return Err("--json must be a JSON object".into());
        }
    }

    Ok(body)
}

fn finish_login(
    resp: client::Response,
    _parsed: &args::ParsedArgs,
    body: Map<String, Value>,
) -> Result<(), String> {
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
        cfg.email = body.get("email").and_then(Value::as_str).map(String::from);
        config::save(&cfg).map_err(|e| format!("failed to save token: {e}"))?;
        println!("Login successful. Token saved to {}", config::config_path().display());
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

/// Keep only entries in `data` (when it's an array) that contain `needle`.
fn filter_data(out: &mut Value, needle: &str) {
    let needle = needle.to_lowercase();
    if let Some(arr) = out.get_mut("data").and_then(Value::as_array_mut) {
        arr.retain(|item| {
            serde_json::to_string(item)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle)
        });
    }
}

fn print_response(out: &Value, parsed: &args::ParsedArgs, status: u16) -> Result<(), String> {
    let text = if parsed.has("raw") {
        serde_json::to_string(out).unwrap_or_default()
    } else {
        serde_json::to_string_pretty(out).unwrap_or_default()
    };
    println!("{text}");

    // Exit non-zero when the API reports failure (code != 1) or HTTP is not 2xx.
    let code = out.get("code").and_then(Value::as_i64);
    if !(200..300).contains(&status) {
        exit(1);
    }
    if let Some(code) = code {
        if code != 1 {
            exit(1);
        }
    }
    Ok(())
}

fn print_command_help(command: &Command) {
    println!("gfit-cli {} — {}", command.name, command.desc);
    if !command.aliases.is_empty() {
        println!("\n  Aliases: {}", command.aliases.join(", "));
    }
    println!();
    let is_local = matches!(command.special, Special::Status | Special::Update);
    if is_local {
        println!("  (local command — does not call the GFIT API or require login)");
    } else {
        let method = if command.method == Method::Multipart { "POST (multipart/form-data)" } else { "POST" };
        println!("  Endpoint: {method} {}", command.path);
        println!("  Auth:     {}", if command.auth { "required" } else { "none" });
    }

    if command.params.is_empty() {
        println!("\n  Parameters: (none)");
    } else {
        println!("\n  Parameters:");
        let width = command.params.iter().map(|p| p.name.len()).max().unwrap_or(0);
        for p in &command.params {
            let tag = if p.required {
                "required".to_string()
            } else if let Some(d) = &p.fixed {
                format!("default: {d}")
            } else {
                "optional".to_string()
            };
            println!(
                "    --{:<width$}  {:<7} [{}]  {}",
                p.name,
                p.ty.label(),
                tag,
                p.desc,
                width = width
            );
        }
    }

    if is_local {
        println!("\n  Flags:");
        println!("    -h, --help       Show this help");
    } else {
        println!("\n  Common flags:");
        println!("    --json '{{...}}'   Merge a raw JSON object into the request body");
        println!("    --dry-run        Print the request instead of sending it");
        println!("    --raw            Print the response as compact JSON");
        println!("    -h, --help       Show this help");
    }

    println!("\n  Example:");
    println!("    {}", example_for(command));
}

fn example_for(command: &Command) -> String {
    let mut s = format!("gfit-cli {}", command.name);
    for p in command.params.iter().filter(|p| p.required) {
        let sample = match p.ty {
            PType::Num => "1".to_string(),
            PType::Bool => "true".to_string(),
            PType::Json => "'{}'".to_string(),
            PType::Str => format!("<{}>", p.name),
        };
        s.push_str(&format!(" --{} {}", p.name, sample));
    }
    s
}

fn print_global_help(reg: &[Command]) {
    println!("gfit-cli {VERSION} — command-line client for the GFIT wellness API\n");
    println!("USAGE:");
    println!("    gfit-cli <command> [--flag value ...]");
    println!("    gfit-cli <command> -h          Show a command's API documentation");
    println!("    gfit-cli -h | --version\n");
    println!("GETTING STARTED:");
    println!("    gfit-cli auth.login --email you@example.com --password secret");
    println!("    gfit-cli auth.status");
    println!("    gfit-cli coach.clients --search 'john'\n");
    println!("CONFIG:");
    println!("    Token + email saved to {}", config::config_path().display());
    println!("    Override base URL with GFIT_API_URL, config path with GFIT_CONFIG.\n");

    println!("COMMANDS ({} total):", reg.len());
    let mut last_group = "";
    let name_width = reg.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in reg {
        let g = c.group();
        if g != last_group {
            println!();
            last_group = g;
        }
        let alias = if c.aliases.is_empty() {
            String::new()
        } else {
            format!("  (alias: {})", c.aliases.join(", "))
        };
        println!("  {:<width$}  {}{}", c.name, c.desc, alias, width = name_width);
    }
}

/// Cheap nearest-command suggestion by shared prefix / substring.
fn suggest<'a>(reg: &'a [Command], name: &str) -> Option<&'a str> {
    let lower = name.to_lowercase();
    reg.iter()
        .map(|c| c.name.as_str())
        .find(|n| n.contains(&lower) || lower.contains(*n))
        .or_else(|| {
            let prefix = lower.split('.').next().unwrap_or(&lower);
            reg.iter().map(|c| c.name.as_str()).find(|n| n.starts_with(prefix))
        })
}
