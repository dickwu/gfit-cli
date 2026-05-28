# gfit-cli

A command-line client for the **GFIT wellness API** (`https://api.gfitwellness.ca`),
written in Rust. It mirrors every API call the `workout` desktop app makes —
**143 commands** covering auth, admin tools, coaching, nutrition, workouts,
check-ins, metrics (weight / step / sleep / text logs), invoices, and Nowledge.

Every command is self-documenting: run it with `-h` to print its full API
documentation (endpoint, auth requirement, and every parameter with type and
meaning).

## Install

Download the binary for your platform from the
[latest release](https://github.com/dickwu/gfit-cli/releases/latest) — assets are
named `gfit-cli-<target>` (e.g. `gfit-cli-aarch64-apple-darwin`):

```bash
# macOS (Apple Silicon) example
curl -fsSL -o gfit-cli https://github.com/dickwu/gfit-cli/releases/latest/download/gfit-cli-aarch64-apple-darwin
chmod +x gfit-cli
sudo mv gfit-cli /usr/local/bin/
gfit-cli --version
```

Available targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.

Once installed, keep it current with `gfit-cli self.update` (see [Updating](#updating)).

## Build from source

```bash
cargo build --release
# binary: target/release/gfit-cli
# (optionally) install it on your PATH:
install -m755 target/release/gfit-cli /usr/local/bin/gfit-cli
```

## Quick start

```bash
# 1. Log in — the token is saved to ~/.config/gfit.json
gfit-cli auth.login --email you@example.com --password 'secret'

# 2. Check who you are / where state lives
gfit-cli auth.status

# 3. Call any endpoint
gfit-cli coach.clients --search 'john'
gfit-cli admin.members --type all --page 1
gfit-cli coach.weight.log-list --uid 42 --start_date 2026-01-01
```

## Authentication is required

Every command talks to the API **only after you log in**. With no saved token,
the *only* things that run are `auth.login`, the local `auth.status` / `whoami`,
and `-h` / `--version`. Any other command exits immediately:

```
$ gfit-cli coach.clients
error: not logged in — run `gfit-cli auth.login --email <e> --password <p>` first
```

The login check happens before argument validation, so you always get the
"not logged in" message rather than a confusing missing-parameter error.
`--dry-run` is exempt — it never calls the API, so you can inspect any request
offline (it prints `NOT logged in — this call would fail`).

## Command naming

Commands are **dot-named** as `group.action`, derived from the API path:

| API path prefix | Command group | Notes |
|-----------------|---------------|-------|
| `auth/…`        | `auth.*`      | login / logout / info |
| `admin/…`       | `admin.*`     | admin-tool surface |
| `staff/…`       | `coach.*`     | the app's "coach" views map to `staff/*` endpoints |

Examples:

```
auth/login                       -> auth.login
admin/user/list                  -> admin.users        (alias: admin.members)
staff/workout/client/list        -> coach.clients
staff/client/weight/log/list     -> coach.weight.log-list
```

Run `gfit-cli` with no arguments to see the full, grouped command list. A few
ergonomic **aliases** exist (`admin.members`, `whoami`, …) and show up next to
their canonical command.

## Per-command API docs (`-h`)

```
$ gfit-cli auth.login -h
gfit-cli auth.login — Log in with email + password; saves the returned token to ~/.config/gfit.json

  Endpoint: POST auth/login
  Auth:     none

  Parameters:
    --email     string  [required]  Account email
    --password  string  [required]  Account password

  Common flags:
    --json '{...}'   Merge a raw JSON object into the request body
    --dry-run        Print the request instead of sending it
    --raw            Print the response as compact JSON
    -h, --help       Show this help
```

## Global flags (available on every command)

| Flag | Purpose |
|------|---------|
| `--dry-run` | Print the request (URL, auth, body) instead of sending it. Does **not** require a token. |
| `--json '{...}'` | Merge a raw JSON object into the request body (applied last, overrides other flags). |
| `--raw` | Print the response as compact JSON instead of pretty-printed. |
| `-h`, `--help` | Show the command's API documentation. |
| `-V`, `--version` | Print the version. |

### Passthrough parameters

Beyond documented parameters, **any** `--key value` flag is added to the JSON
request body. Values are auto-typed: `true`/`false` → bool, integers/decimals →
numbers, `{...}`/`[...]` → parsed JSON, everything else → string. This means
every endpoint is fully callable even where a field isn't formally documented:

```bash
gfit-cli admin.food.create --name Apple --calories 95 --carbs 25 \
  --protein 0 --fat 0 --fibre 4 --weight 182 --measure 'medium' --category Fruit
```

## Updating

`gfit-cli` can upgrade itself from GitHub Releases — no package manager needed:

```bash
gfit-cli self.update --check     # report current vs latest; install nothing
gfit-cli self.update             # download + replace this binary if a newer one exists
gfit-cli self.update --force     # reinstall the latest even if already current
gfit-cli self.update --insecure  # skip SHA-256 checksum verification (not recommended)
```

It fetches the release asset matching the running platform, **verifies it against
the published `<asset>.sha256` checksum**, then swaps the binary in place
atomically (rolling back on failure). Downloads only follow redirects to
`https://*.github.com` hosts, and a mismatched or missing checksum aborts the
install (override with `--insecure`). If `gfit-cli` lives in a protected
directory (e.g. `/usr/local/bin`), run the update with `sudo`. This is a local
command — it talks to GitHub, not the GFIT API, so it needs no login.

## Configuration

State is stored at **`~/.config/gfit.json`**:

```json
{
  "token": "…",
  "email": "you@example.com"
}
```

Environment overrides:

| Variable | Effect |
|----------|--------|
| `GFIT_CONFIG` | Use a different config file path. |
| `GFIT_API_URL` | Override the API base URL (e.g. for staging). |

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success (HTTP 2xx and API `code == 1`). |
| `1` | API or transport error (non-2xx, API `code != 1`, missing required arg, bad input). |
| `2` | Unknown command. |

## Project layout

```
build.rs        bakes the build target triple in (GFIT_TARGET) for self-update asset matching
src/
  main.rs       entry point: dispatch, body building, help rendering, login persistence
  registry.rs   single source of truth — every endpoint as a dot-named command + params
  client.rs     HTTP layer (reqwest blocking; JSON + multipart POST; GitHub get/download)
  config.rs     ~/.config/gfit.json load/save
  args.rs       --key value / --key=value / bare-flag parser
  update.rs     self-update: check GitHub Releases + replace the binary in place
tests/
  cli.rs        end-to-end tests driving the built binary (dry-run, never networks)
.github/workflows/
  release.yml   on tag push, build macOS/Linux binaries and attach them to the release
```

The registry is the only thing you edit to add or adjust endpoints; `main.rs`
drives entirely off it for dispatch, `-h` docs, body building, and auth.

## Tests

```bash
cargo test
```

Covers coercion, registry invariants (unique names, no alias collisions, auth
rules), and end-to-end CLI behavior via `--dry-run` (no network access).
