# gfit-cli skills

Agent Skills that wrap `gfit-cli` for everyday coaching workflows. They're packaged in the
[`npx skills`](https://github.com/vercel-labs/skills) flat layout (`skills/<name>/SKILL.md`),
so they install straight from this repo into Claude (and other agents).

## Available skills

| Skill | What it does |
|-------|--------------|
| [`gfit-weekly-checkin`](./gfit-weekly-checkin/SKILL.md) | Reviews the coach's client roster, finds who hasn't submitted their weekly check-in, and drafts tiered follow-up emails in Gmail (drafts only — never sends). Built for the recurring Tuesday-morning check-in sweep. |

## Install

```bash
# Interactive — pick which Claude app(s) to install into when prompted:
npx skills add dickwu/gfit-cli --skill gfit-weekly-checkin

# Or target Claude Code directly:
npx skills add dickwu/gfit-cli --skill gfit-weekly-checkin -a claude-code

# Add -g to install it globally (available in every project), instead of just here:
npx skills add dickwu/gfit-cli --skill gfit-weekly-checkin -a claude-code -g
```

If your Claude app reads skills from a folder and isn't offered by `npx skills`, you can
also just copy the `gfit-weekly-checkin/` folder into that app's skills directory.

## What your Claude needs to run `gfit-weekly-checkin`

1. **`gfit-cli` installed and runnable** — the skill calls it from the shell. See
   [`gfit-weekly-checkin/references/gfit-cli.md`](./gfit-weekly-checkin/references/gfit-cli.md)
   for the one-line binary install, then `gfit-cli auth.login` to sign in as the coach.
2. **Gmail connected**, with the ability to create drafts (so the skill can drop follow-ups
   into your Drafts folder).

## How to run it

Once installed, just ask in plain language:

> "Run the weekly check-in review and draft follow-ups for anyone who hasn't checked in."

or simply **"run the Tuesday check-in."** Claude pulls the roster, classifies each client by
how long they've been quiet, drafts a tiered email per client, and hands you a summary
table. Everything lands in **Gmail Drafts for you to review and send** — the skill never
sends mail itself.

To automate the Tuesday 8:00 AM Central run, see the *Scheduling* section in
[`references/gfit-cli.md`](./gfit-weekly-checkin/references/gfit-cli.md).
