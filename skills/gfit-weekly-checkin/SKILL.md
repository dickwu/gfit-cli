---
name: gfit-weekly-checkin
description: >-
  Review GFIT coaching clients' weekly check-in submissions and draft tiered
  follow-up emails in Gmail for anyone who hasn't checked in. Use this skill
  whenever the user wants to run the weekly client check-in review, find out who
  has or hasn't submitted their weekly check-in, follow up with clients who went
  quiet, draft accountability or "how's your training going" reminder emails, or
  do the recurring Tuesday-morning client check-in sweep. Triggers on phrases
  like "weekly check-in review", "who hasn't checked in this week", "draft
  check-in follow-ups", "client accountability emails", "remind clients to submit
  their check-in", or "run the Tuesday check-in". It reads the roster with the
  gfit-cli command-line tool and creates Gmail DRAFTS only — it never sends mail.
metadata:
  version: 1.0.0
---

# GFIT Weekly Check-in Follow-up

Help a GFIT coach stay on top of client accountability. Each week you:

1. pull the coach's client roster from the GFIT API (via the `gfit-cli` tool),
2. find who has **not** submitted their weekly check-in and how long they've been quiet,
3. draft a personalised follow-up email **as a Gmail draft** for each of them, with the
   tone matched to how many weeks they've missed, and
4. hand the coach a short report so they can glance over the drafts and send them.

This exists so the coach doesn't have to scroll a long client list every Tuesday and
hand-type the same nudge over and over. You do the sorting and the first draft; they
keep the judgment and the send button.

## The one rule that matters most

**You create Gmail _drafts_. You never send email, and you never auto-send.** Every
message waits in the coach's Drafts folder for them to read, tweak, and send. This keeps
a human in the loop on every client relationship — which is the whole point of an
accountability touch. If the only thing you can do is *send* (no draft capability), stop
and tell the coach rather than sending.

And **never draft anything for a client who already checked in this week.** Getting a
"where have you been?" note right after checking in is the fastest way to annoy a good
client. When in doubt, leave them out and say so in the report.

## Before you start: prerequisites

Check these in order. If one fails, fix it (or tell the coach how) before moving on.

1. **`gfit-cli` is installed and you can run shell commands.** Run `gfit-cli --version`.
   If it's missing, see `references/gfit-cli.md` → *Install* (a single binary download).
2. **The coach is logged in.** Run `gfit-cli auth.status`. If it prints `(not logged in)`
   / `token: absent`, run `gfit-cli auth.login` (opens a browser sign-in) and wait for it
   to finish. Never guess or ask for raw credentials.
3. **Gmail is connected** in this Claude app, with the ability to create drafts. If you
   have no Gmail draft capability, stop and ask the coach to connect Gmail first.

## Step 1 — Establish "this week" in Central Time

The check-in cadence is weekly and the coach runs this Tuesday morning, US Central
(`America/Chicago`, i.e. CST/CDT). Work out today's date in Central Time and use it as
the reference point for "how many days since this client last checked in." All thresholds
below are in **days**, so daylight-saving never trips you up.

Classification is based purely on **days since each client's last check-in** — it doesn't
depend on which weekday you run it. A catch-up run on a Wednesday or Friday behaves exactly
like the Tuesday one; the "Tuesday morning" timing is just the usual cadence, not a
week-boundary rule.

## Step 2 — Pull the client roster

```bash
gfit-cli coach.clients --raw
```

This returns the **logged-in coach's assigned clients** as JSON (success is `code == 1`;
the clients live in the `data` payload). Inspect the real fields in the response — per
client you want: a numeric **id**, a **name** (first/last or full), an **email**, and, if
present, a **status** and a **last check-in / last activity** date. Field names can vary;
match by meaning, and read `references/gfit-cli.md` if the shape is unfamiliar.

Skip — and note in the report — any client who is **inactive / cancelled / paused** (if
the roster exposes such a status) or who **has no email address** (you can't draft to
them).

## Step 3 — Find each client's most recent check-in

If the roster already carries a reliable "last check-in" date per client, use it and skip
the per-client calls. Otherwise, for each active client:

```bash
gfit-cli coach.checkin.list --id <client_id> --raw
```

This returns that member's recent check-ins. Take the **most recent** check-in's date. If
the list is empty, treat them as "never checked in" (handled in Step 4). One call per
client is fine; on a big roster you can read several in a row, but keep the drafting in
Step 6 reviewable.

## Step 4 — Classify each client

Compute **days since last check-in** (Central Time) and bucket each client. Full rules and
edge cases are in `references/classification.md`; the short version:

| Bucket | Days since last check-in | Action |
|--------|--------------------------|--------|
| Current | ≤ 7 | **No email** — they're up to date |
| Missed this week | 8–14 | **Tier 1** — friendly nudge |
| Missed a few weeks | 15–28 | **Tier 2** — caring check-in + offer to help |
| Long disengaged / never | > 28, or never checked in | **Tier 3** — re-engagement |

Count **whole elapsed days** (floor) in Central Time and read the boundaries inclusively as
written: a check-in exactly 7 days ago is still *Current*, exactly 14 → Tier 1, exactly 28 →
Tier 2; a check-in today is 0 days (Current).

Brand-new clients (joined within the last 7 days) who haven't checked in yet: skip them —
it's too early to chase.

## Step 5 — Avoid duplicate drafts (so re-runs are safe)

Before drafting for a client, check whether a follow-up draft to that same client already
exists from this run-week: search Gmail Drafts for a message **to their email address**
whose subject matches one of the tier subject lines in `assets/email-templates.md` and was
**created in the last 7 days**. If one exists, skip and note it. This makes the skill safe
to run twice in a morning (or to re-run after a crash) without piling up duplicate drafts.

## Step 6 — Draft the emails

For every client in Tier 1 / 2 / 3, create **one Gmail draft** addressed to that client,
using the matching template in `assets/email-templates.md`. Personalise it:

- `{{first_name}}` → the client's first name (first token of their name).
- `{{coach_name}}` → the coach's name. Default **Grant** unless the coach says otherwise.
- `{{checkin_url}}` → the weekly check-in link. Default
  **https://www.gfitwellness.ca/weekly-check-in**.

Keep the coach's warm, personal voice — these should read as if Grant typed them himself,
not like a marketing blast. One draft per client. Never send.

## Step 7 — Report back

Print a compact table so the coach can scan and send. One row per client considered:

Use a Tier of `—` for any client you skip, and put the reason in the Draft column, so every
row is accounted for:

```
Name          Email                 Last check-in   Days   Tier             Draft
Peilin Wu     peilin@example.com    2026-05-19       10    1 (this week)    ✅ created
Jordan Lee    jordan@example.com    2026-05-08       21    2 (few weeks)    ✅ created
Sam Diaz      sam@example.com       never            —     3 (re-engage)    ✅ created
Alex Kim      alex@example.com      2026-05-26        3    current          — skipped (up to date)
Nia Newbie    nia@example.com       never            —     —                — skipped (new, joined 2d ago)
Pat Roe       (no email)            never            —     —                ⚠️ skipped (no email)
Gabe Gone     gabe@example.com      cancelled        —     —                — skipped (inactive)
```

Close with the counts (e.g. "Created 3 drafts: 1 Tier-1, 1 Tier-2, 1 Tier-3. 1 up to date,
3 skipped — 1 new, 1 no email, 1 inactive.") and this reminder: **the emails are sitting in
your Gmail Drafts — review and send the ones you want.**

## Customising

- **Tone / copy:** edit `assets/email-templates.md`.
- **Thresholds (what counts as "a few weeks"):** edit `references/classification.md`.
- **Coach name / check-in URL:** the coach can just say "sign them from Coach X" or "use
  this check-in link" when they run the skill, or change the defaults above.

## Scheduling it for Tuesday 8am CST

This skill is the *what to do*; it doesn't schedule itself. To run it automatically every
Tuesday at 8:00 AM Central, see `references/gfit-cli.md` → *Scheduling* — either a Claude
Code scheduled routine/cron, or simply invoking it by hand each Tuesday morning.
