# Classifying clients by check-in recency

The goal is to match the *tone* of the follow-up to how long a client has been quiet. A
client who missed one week needs a light nudge; someone who's been gone a month needs a
warmer, lower-pressure reconnect — not the same chirpy reminder. These buckets encode that.

## Compute "days since last check-in"

1. Get the client's most recent check-in date (Step 3 of the skill).
2. Compute the whole number of days between that date and **today in `America/Chicago`**
   (Central). Use days, not weeks, so daylight-saving and partial weeks don't cause
   off-by-one surprises.
3. Bucket on the day count below.

## The buckets

| Bucket | Days since last check-in | Tier / action | Why |
|--------|--------------------------|---------------|-----|
| **Current** | ≤ 7 | No email | Checked in within the last week — leave them alone. |
| **Missed this week** | 8–14 | **Tier 1** — friendly nudge | One week slipped. A light, encouraging reminder is all that's needed. |
| **Missed a few weeks** | 15–28 | **Tier 2** — caring check-in + offer help | Two-to-four weeks quiet. Show you noticed and open the door to remove a blocker. |
| **Long disengaged / never** | > 28, or no check-in on record | **Tier 3** — re-engagement | A month-plus, or never started. Low-pressure "let's reconnect," not a guilt trip. |

These day thresholds (7 / 14 / 28) are the editable knobs — change them here if the coach
defines "a few weeks" differently. Keep the buckets ordered and non-overlapping.

Count **whole elapsed days** (floor) in Central Time, and read the boundaries inclusively
exactly as written: a check-in **today** is 0 days (Current); **exactly 7** days = Current;
**exactly 14** = Tier 1; **exactly 28** = Tier 2. This removes any ambiguity at the edges.

## Edge cases (handle these explicitly)

- **Never checked in, but a brand-new client.** If the client joined within the last 7 days
  and has no check-in yet, **skip** them — it's too early to chase, and a re-engagement
  note would be confusing. If joined longer ago with no check-in ever, that's **Tier 3**.
- **Never checked in, unknown join date.** Default to **Tier 3** (re-engagement) — it reads
  fine for a long-quiet client and isn't accusatory for a newer one.
- **Inactive / cancelled / paused status.** Skip and note it. Don't chase someone who has
  left or paused the program.
- **No email address.** Can't draft — skip and flag it in the report so the coach can fix
  the record.
- **Multiple check-ins in the same week.** Only the most recent matters; they're Current.
- **Weird or missing dates in the check-in payload.** If you can't parse a reliable date,
  don't guess a tier — list the client under "couldn't classify" in the report and move on.
- **Already has a draft from this week.** Skip (see Step 5 — idempotency) and note it.

## Output of this step

For each client, you should know: `{ id, first_name, email, last_checkin_date | "never",
days_since | null, tier ∈ {none, 1, 2, 3, skip}, skip_reason? }`. That's exactly what the
report table in Step 7 prints, and what Step 6 needs to pick a template.
