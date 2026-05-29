# gfit-cli recipes

End-to-end examples of the discover → build → preview → run → parse loop. They're
patterns to adapt, not rigid scripts. Each assumes you're logged in (`gfit-cli auth.status`).

## 1. Look up a client and read their data

```bash
gfit-cli coach.clients --search 'jordan' --raw      # find them; note the numeric id
gfit-cli coach.checkin.list --id 102 --raw          # recent check-ins
gfit-cli coach.weight.log-list --uid 102 --start_date 2026-01-01 --raw   # a metric range
```

Parse `data` from each. If you don't know a command's params, `gfit-cli <cmd> -h` first
(e.g. metric log-lists take a `--uid` and a date range — confirm the exact names with `-h`).

## 2. Weekly client check-in review → draft follow-up emails

A complete, real task built entirely from the general commands — this is the kind of
workflow the skill is meant to power. Goal: find which of the coach's clients haven't
submitted their weekly check-in and draft a follow-up email to each (as **Gmail drafts**,
never sent), with the tone matched to how long they've been quiet.

**Steps**

1. **Roster:** `gfit-cli coach.clients --raw` → the logged-in coach's clients. Per client
   pull, by meaning, an **id**, **name**, **email**, and any **status** / **join date**.
   Skip (and note) anyone inactive/cancelled or without an email.
2. **Last check-in per client:** for each active client,
   `gfit-cli coach.checkin.list --id <id> --raw` → take the **most recent** check-in's
   date. Empty list = never checked in.
3. **Classify by days since last check-in** (compute in the coach's timezone, e.g.
   `America/Chicago`). A sensible default ladder — adjust to the coach's preference:
   - **≤ 7 days** → up to date, no email.
   - **8–14 days** (missed this week) → *Tier 1*, a light friendly nudge.
   - **15–28 days** (a few weeks) → *Tier 2*, a caring check-in that offers help.
   - **> 28 days or never** → *Tier 3*, a low-pressure re-engagement.
   - Brand-new clients (joined < 7 days ago) with no check-in yet → skip; too early.
4. **Draft, don't send.** Create one **Gmail draft** per client needing follow-up, in the
   coach's voice, personalised by first name, with the check-in link
   (`https://www.gfitwellness.ca/weekly-check-in`) and signed by the coach. Drafts only —
   the coach reviews and sends. Don't email anyone who's up to date.
5. **Report** a table: name, email, last check-in, days, tier, draft created/skipped +
   reason. Remind the coach the drafts are waiting in Gmail.

**Example draft (Tier 1 — missed this week)** — adapt the copy/thresholds to the coach:

```
To: <client email>
Subject: Checking in — how's your week going?

Hi <first name>,

I hope you're having a great week! I just wanted to check in and see how everything is
going with your training and nutrition. Whenever you get a chance, send me a quick update
here: https://www.gfitwellness.ca/weekly-check-in

I'm always here to support you with your health and fitness goals.

Sincerely,
<coach name>
```

For longer silences, warm the tone and lower the pressure: Tier 2 acknowledges it's been a
couple of weeks and offers to clear a blocker ("just reply and tell me what's getting in the
way"); Tier 3 is a genuine "I don't want to lose touch — even a one-line reply helps"
reconnect. Keep them personal, never corporate, one draft per client.

> This whole recipe uses only `coach.clients` + `coach.checkin.list` (both reads) and your
> Gmail draft capability — no writes to GFIT, so it's safe to run any time.

## 3. Pull a metric log range

```bash
gfit-cli coach.weight.log-list --uid 42 --start_date 2026-01-01 --end_date 2026-03-31 --raw
gfit-cli coach.step.log-list   --uid 42 --start_date 2026-01-01 --end_date 2026-03-31 --raw
gfit-cli coach.text.stress     --uid 42 --start_date 2026-01-01 --end_date 2026-03-31 --raw
```

Confirm each command's exact parameter names with `-h` — metric commands share a shape but
verify before assuming. Aggregate the `data` arrays for trends.

## 4. Safely perform a write (preview first!)

Writes hit production — always dry-run, show the request, get a yes, then send.

```bash
# 1) Build + PREVIEW (no token needed, nothing sent):
gfit-cli coach.client-update --id 102 --phase 'Build' --note 'Bumped to build phase' --dry-run
#    → shows: POST staff/workout/client/update  body { "id":102, "phase":"Build", ... }
# 2) Show that to the user, get explicit confirmation.
# 3) Only then run for real, parsing the result:
gfit-cli coach.client-update --id 102 --phase 'Build' --note 'Bumped to build phase' --raw
```

**Email/destructive calls need the same care, louder:** `coach.workout.notice`,
`admin.workout.notice`, and `plan.meals.notice` **send email to clients**; any `*.delete`
removes data. Dry-run, confirm, and for deletes read the target first to be sure of the id.
