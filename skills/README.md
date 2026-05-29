# gfit-cli skills

Agent Skills packaged in the [`npx skills`](https://github.com/vercel-labs/skills) flat
layout (`skills/<name>/SKILL.md`), so they install straight from this repo into Claude (and
other supported agents).

## Available skills

| Skill | What it does |
|-------|--------------|
| [`gfit-cli`](./gfit-cli/SKILL.md) | Makes Claude fluent and **safe** with `gfit-cli` for *any* GFIT API operation: the `group.action` command model, the discovery loop (`gfit-cli` + `<cmd> -h`), authentication, the global flags (`--dry-run` / `--raw` / `--json`), arbitrary passthrough parameters, the `code == 1` / `data` response envelope, exit codes, and a confirm-before-write workflow. Ships worked recipes — including a **weekly client check-in review** that finds who hasn't checked in and drafts follow-up emails. |

## Setup (one time)

The skill *drives the `gfit-cli` binary*, so set the tool up and log in first, then install
the skill.

### 1. Install the `gfit-cli` binary

```bash
# macOS (Apple Silicon) example:
curl -fsSL -o gfit-cli https://github.com/dickwu/gfit-cli/releases/latest/download/gfit-cli-aarch64-apple-darwin
chmod +x gfit-cli && sudo mv gfit-cli /usr/local/bin/
gfit-cli --version
```

Other targets (`x86_64-apple-darwin`, `{x86_64,aarch64}-unknown-linux-gnu`) and a
build-from-source option are in the [main README](../README.md#install).

### 2. Log in

```bash
gfit-cli auth.login      # opens a browser sign-in; saves the token to ~/.config/gfit.json
gfit-cli auth.status     # confirm — should show your email and "token: present"
```

You act as **whoever logs in here** — e.g. `coach.clients` returns *that* coach's clients —
so sign in as the coach who'll use the skill. Headless / scriptable alternative:

```bash
gfit-cli auth.login --email you@example.com --password 'secret'
```

### 3. Install the skill

```bash
# Interactive — pick which Claude app(s) to install into when prompted:
npx skills add dickwu/gfit-cli --skill gfit-cli

# Or target Claude Code directly; add -g for a global (all-projects) install:
npx skills add dickwu/gfit-cli --skill gfit-cli -a claude-code
npx skills add dickwu/gfit-cli --skill gfit-cli -a claude-code -g
```

If your Claude app reads skills from a folder and isn't offered by `npx skills`, just copy
the `gfit-cli/` folder into that app's skills directory.

### 4. Connect Gmail (only for email-drafting recipes)

The weekly check-in recipe drafts follow-up emails, so connect **Gmail** in your Claude app
with permission to create drafts. Pure data tasks — listing clients, reading logs — don't
need this.

## How to use

Just ask in plain language — the skill triggers on GFIT operations even if you don't say
"gfit-cli":

> "Pull my clients", "who hasn't checked in this week and draft follow-ups", "list weight
> logs for user 42 this quarter", "search foods for chicken", "preview that meal-plan
> publish before I send it."

Claude will discover the right command, preview any write with `--dry-run`, and parse the
`data` payload for you — drafting emails only as Gmail drafts, never sending.
