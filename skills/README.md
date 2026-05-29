# gfit-cli skills

Agent Skills packaged in the [`npx skills`](https://github.com/vercel-labs/skills) flat
layout (`skills/<name>/SKILL.md`), so they install straight from this repo into Claude (and
other supported agents).

## Available skills

| Skill | What it does |
|-------|--------------|
| [`gfit-cli`](./gfit-cli/SKILL.md) | Makes Claude fluent and **safe** with `gfit-cli` for *any* GFIT API operation: the `group.action` command model, the discovery loop (`gfit-cli` + `<cmd> -h`), authentication, the global flags (`--dry-run` / `--raw` / `--json`), arbitrary passthrough parameters, the `code == 1` / `data` response envelope, exit codes, and a confirm-before-write workflow. Ships worked recipes — including a **weekly client check-in review** that finds who hasn't checked in and drafts follow-up emails. |

## Install

```bash
# Interactive — pick which Claude app(s) to install into when prompted:
npx skills add dickwu/gfit-cli --skill gfit-cli

# Or target Claude Code directly, and -g for a global (all-projects) install:
npx skills add dickwu/gfit-cli --skill gfit-cli -a claude-code
npx skills add dickwu/gfit-cli --skill gfit-cli -a claude-code -g
```

If your Claude app reads skills from a folder and isn't offered by `npx skills`, just copy
the `gfit-cli/` folder into that app's skills directory.

## What your Claude needs

1. **`gfit-cli` installed and runnable** from the shell, and logged in
   (`gfit-cli auth.login`). The skill's own *Authentication* section covers this; the
   binary install is in the repo [README](../README.md#install).
2. **Only for recipes that draft email** (e.g. the check-in review): **Gmail connected**,
   with the ability to create drafts.

## How to use

Just ask in plain language — the skill triggers on GFIT operations even if you don't say
"gfit-cli":

> "Pull my clients", "who hasn't checked in this week and draft follow-ups", "list weight
> logs for user 42 this quarter", "search foods for chicken", "preview that meal-plan
> publish before I send it."

Claude will discover the right command, preview any write with `--dry-run`, and parse the
`data` payload for you — drafting emails only as Gmail drafts, never sending.
