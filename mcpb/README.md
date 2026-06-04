# GFIT CLI — Claude Desktop bundle (`.mcpb`)

An [MCP Bundle](https://github.com/anthropics/mcpb) that wraps the `gfit-cli` binary so
**Claude Desktop** can drive the GFIT wellness API. One-click install: drag the built
`gfit-cli.mcpb` into Claude Desktop → **Settings → Extensions**.

## Tools it adds

| Tool | Purpose |
|------|---------|
| `gfit_list_commands` | List every gfit-cli command, grouped (the full ~144-command surface). |
| `gfit_help` | Show one command's API docs — endpoint, auth, every parameter. |
| `gfit_auth_status` | Check whether gfit-cli is logged in. |
| `gfit_login` | Open the GFIT browser sign-in page (no password passes through Claude). |
| `gfit_run` | Run any command. Reads run directly; **writes/destructive commands return a `--dry-run` preview unless `confirm: true`** (`*.notice` sends client email, `*.delete` removes data). **If you're not logged in, it opens the sign-in page instead of failing** — sign in, then retry. |

## Prerequisites

1. **`gfit-cli` installed** (the binary this server shells out to) — see the repo
   [README](../README.md#install).
2. **Logged in.** The MCP server reuses the saved token at `~/.config/gfit.json`. You no
   longer have to log in up front: the first time you run a command while logged out (or
   any time you call `gfit_login`), the server starts the **browser sign-in** for you and
   returns the page URL — sign in there and the token is saved automatically. Your password
   never passes through Claude. (You can still pre-seed it with `gfit-cli auth.login` in a
   terminal if you prefer.)

After install, configure the **gfit-cli path** (default `gfit-cli` if on your PATH) and
optionally the **API base URL** in the extension's settings.

## Build the `.mcpb`

```bash
cd mcpb
npm install --omit=dev          # bundle node_modules (the MCP SDK)
npx @anthropic-ai/mcpb pack . gfit-cli.mcpb   # produce the installable bundle
```

That yields `gfit-cli.mcpb` — a zip of `manifest.json` + `server/` + `node_modules/`.
`npx @anthropic-ai/mcpb validate manifest.json` checks the manifest on its own.

## Why a CLI wrapper (not a reimplemented server)

The server spawns `gfit-cli` and exposes a thin, safe surface over it, so every endpoint,
the auth model, and `--dry-run` come for free and stay in sync with the CLI. It resolves
the binary from common install dirs (`/usr/local/bin`, `/opt/homebrew/bin`, `~/.cargo/bin`,
`~/.local/bin`) because Claude Desktop can launch with a minimal `PATH`.
