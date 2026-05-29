//! End-to-end CLI tests. These run the built binary with `--dry-run` / `-h`,
//! so they never touch the network. Cargo provides the binary path via
//! `CARGO_BIN_EXE_gfit-cli`.

use std::process::Command;

fn bin() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_gfit-cli"));
    // Isolate from any real ~/.config/gfit.json so auth state is deterministic.
    c.env("GFIT_CONFIG", "/tmp/gfit-cli-test-config.json");
    c.env_remove("GFIT_API_URL");
    c
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Out {
    let o = bin().args(args).output().expect("failed to run binary");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

#[test]
fn global_help_lists_commands() {
    let o = run(&["-h"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("COMMANDS"));
    assert!(o.stdout.contains("auth.login"));
    assert!(o.stdout.contains("coach.clients"));
}

#[test]
fn version_flag() {
    let o = run(&["--version"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("gfit-cli"));
}

#[test]
fn command_help_shows_api_doc() {
    let o = run(&["auth.login", "-h"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("Endpoint: POST auth/login"));
    assert!(o.stdout.contains("Auth:     none"));
    assert!(o.stdout.contains("--email"));
    assert!(o.stdout.contains("--password"));
}

#[test]
fn alias_resolves_to_canonical_command() {
    // admin.members is an alias for admin.users
    let o = run(&["admin.members", "-h"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("admin.users"));
    assert!(o.stdout.contains("admin/user/list"));
}

#[test]
fn login_dry_run_builds_body() {
    let o = run(&[
        "auth.login",
        "--email",
        "me@example.com",
        "--password",
        "p@ss word",
        "--dry-run",
    ]);
    assert_eq!(o.code, 0);
    assert!(o
        .stdout
        .contains("POST https://api.gfitwellness.ca/auth/login"));
    assert!(o.stdout.contains("\"email\": \"me@example.com\""));
    assert!(o.stdout.contains("\"password\": \"p@ss word\""));
    assert!(o.stdout.contains("auth: (none)"));
}

#[test]
fn dry_run_on_auth_endpoint_without_token_succeeds() {
    // --dry-run must not require being logged in.
    let o = run(&["coach.clients", "--search", "x", "--dry-run"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("staff/workout/client/list"));
    assert!(o.stdout.contains("NOT logged in"));
}

#[test]
fn unknown_command_exits_2_with_suggestion() {
    let o = run(&["coach.client"]);
    assert_eq!(o.code, 2);
    assert!(o.stderr.contains("unknown command"));
    assert!(o.stderr.contains("coach.clients")); // suggestion
}

#[test]
fn missing_required_param_errors() {
    let o = run(&["admin.food.delete", "--dry-run"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("missing required --id"));
}

#[test]
fn json_typed_param_sends_real_array() {
    let o = run(&[
        "admin.meal.update",
        "--uuid",
        "ABC",
        "--content",
        "[{\"meal\":\"breakfast\"}]",
        "--dry-run",
    ]);
    assert_eq!(o.code, 0);
    // content rendered as a nested JSON array, not a quoted string
    assert!(o.stdout.contains("\"content\": ["));
    assert!(o.stdout.contains("\"meal\": \"breakfast\""));
}

#[test]
fn passthrough_flag_becomes_body_field() {
    let o = run(&[
        "coach.search",
        "--name",
        "x",
        "--undocumented_flag",
        "42",
        "--dry-run",
    ]);
    assert_eq!(o.code, 0);
    // undocumented passthrough flag is coerced (int) and included
    assert!(o.stdout.contains("\"undocumented_flag\": 42"));
}

#[test]
fn json_override_merges_last() {
    let o = run(&[
        "coach.search",
        "--name",
        "x",
        "--json",
        "{\"name\":\"override\",\"page\":2}",
        "--dry-run",
    ]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("\"name\": \"override\""));
    assert!(o.stdout.contains("\"page\": 2"));
}

#[test]
fn networked_command_blocked_when_not_logged_in() {
    // A previously-"public" endpoint must now also require login.
    let o = run(&["admin.food.categories"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("not logged in"));
}

#[test]
fn login_runs_without_token() {
    // auth.login is the one networked command allowed without a token (dry-run proves
    // it builds + would send without the login gate firing).
    let o = run(&[
        "auth.login",
        "--email",
        "a@b.c",
        "--password",
        "x",
        "--dry-run",
    ]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("auth/login"));
}

#[test]
fn login_help_describes_browser_flow() {
    // With no --email/--password, auth.login opens a browser sign-in page; the help
    // text advertises that and lists both credentials as optional. Only -h is asserted
    // here — the no-args path binds a local server and waits, so it is verified
    // out-of-band, never in `cargo test`.
    let o = run(&["auth.login", "-h"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.to_lowercase().contains("browser"));
    assert!(o.stdout.contains("--email"));
    assert!(o.stdout.contains("--password"));
    assert!(o.stdout.contains("[optional]"));
}
