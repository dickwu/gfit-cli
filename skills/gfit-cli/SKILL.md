---
name: gfit-cli
description: >-
  Drive the `gfit-cli` command-line client for the GFIT wellness API
  (`api.gfitwellness.ca`) confidently and safely. Use this skill whenever a task
  involves gfit-cli, GFIT coaching/admin data, or the GFIT API from the shell —
  listing or updating clients, coaches, check-ins, meal or workout plans,
  nutrition, foods, splits, metrics (weight / step / sleep / text logs),
  invoices, or Nowledge. It teaches the command model, how to discover any
  command and its parameters, authentication, the global flags
  (`--dry-run` / `--raw` / `--json`), arbitrary passthrough parameters, the
  response envelope, exit codes, and a safe workflow for write/destructive calls.
  Reach for it even when the user only names a GFIT operation without saying
  "gfit-cli" — e.g. "pull my clients", "who hasn't checked in this week", "list
  weight logs for user 42", "publish that meal plan", "search foods for chicken".
metadata:
  version: 1.0.0
---

# Using gfit-cli

`gfit-cli` is a single-binary command-line client for the **GFIT wellness API**. It
mirrors every call the `workout` desktop app makes — **144 commands** spanning auth,
admin tools, coaching, nutrition, workouts, check-ins, metrics, invoices, and Nowledge.
This skill makes you fluent and careful with it, for *any* operation — not one task.

The core idea: **you don't need to memorise 144 commands. You need the model and the
discovery loop below.** With those, every endpoint is reachable and self-documenting.

## The command model

Commands are **dot-named** `group.action`, derived directly from the API path:

| Group | Maps to | What's there |
|-------|---------|--------------|
| `auth.*` | `auth/…` | login, logout, account info, local status |
| `self.*` | (local) | `self.update` — upgrade the binary from GitHub |
| `admin.*` | `admin/…` | the full admin-tool surface (clients, users, meals, foods, splits, workouts, videos, levers, uploads, Nowledge) |
| `coach.*` | `staff/…` | the app's **coach** views — assigned clients, check-ins, plans, metrics, invoices, emails |
| `plan.*` | (member plans) | uploaded meal plans + notices |

So `staff/member/checkin` is `coach.checkin.list`; `admin/user/list` is `admin.users`.
The full group/command map and how to navigate it is in
[`references/command-map.md`](references/command-map.md).

## The golden rule: discover, don't guess

Two commands answer almost every "how do I…" question. Use them before assuming
anything — they're free, offline-friendly, and authoritative:

```bash
gfit-cli                       # full grouped list of all 144 commands
gfit-cli <command> -h          # one command's endpoint, auth, every parameter (type + meaning), example
```

`-h` is the source of truth for a command. Always check it before calling something you
haven't used. Example:

```
$ gfit-cli coach.checkin.list -h
gfit-cli coach.checkin.list — Get a member's recent check-ins
  Endpoint: POST staff/member/checkin
  Auth:     required
  Parameters:
    --id  string  [required]  User id
```

## Authentication

Every API command needs a saved token. Check first, log in if needed:

```bash
gfit-cli auth.status           # who am I / is a token saved? (alias: whoami) — no token needed
gfit-cli auth.login            # browser sign-in (keeps the password out of shell history)
gfit-cli auth.login --email you@example.com --password 'secret'   # scriptable / headless
```

With no token, **only** `auth.login`, `auth.status`, `-h`, and `--version` run; everything
else exits with `not logged in` (the login check happens *before* argument validation, so
you get that message rather than a confusing missing-parameter one). The token lives in
`~/.config/gfit.json`. You act as **whoever is logged in** — e.g. `coach.clients` returns
*that* coach's assigned clients.

## Global flags (on every command)

| Flag | Use it to |
|------|-----------|
| `--dry-run` | Print the exact request (URL, auth, body) **without sending** — needs no token. Your preview tool. |
| `--raw` | Print the response as compact JSON instead of pretty-printed — use whenever you'll parse output. |
| `--json '{...}'` | Merge a raw JSON object into the request body (applied last, overrides other flags). |
| `-h`, `--help` | The command's API docs. |
| `-V`, `--version` | Print the version. |

## Passthrough parameters — every field is reachable

Beyond documented parameters, **any** `--key value` flag is added to the JSON body, with
the value auto-typed: `true`/`false` → bool, integers/decimals → number, `{...}`/`[...]` →
parsed JSON, everything else → string. So you can set fields even where `-h` doesn't list
them:

```bash
gfit-cli admin.food.create --name Apple --calories 95 --carbs 25 --protein 0 --category Fruit
```

If you're unsure of a body shape, build it explicitly with `--json '{...}'` and confirm
with `--dry-run`.

## Reading responses

The API returns an envelope: **success is HTTP 2xx *and* `code == 1`**, with the useful
payload under **`data`**. If `code != 1`, it's an API-level error — surface the `msg` and
stop; don't invent data. Pair this with `--raw` for clean parsing:

```bash
gfit-cli coach.clients --raw    # {"code":1,"data":[ ...clients... ], ...}
```

**Exit codes:** `0` success · `1` API/transport/usage error · `2` unknown command.

## Safe workflow for writes and destructive actions

Most `admin.*` and `coach.*` write commands hit **production data**. Be deliberate:

- **Reads are safe** — `*.list`, `*.search`, `*.info`, `*.status`, `*.details`, `*.home`,
  `coach.checkin.list`, metric `*.log-list`. Run these freely to explore.
- **Writes need a preview + confirmation** — `*.create`, `*.update`, `*.generate`,
  `*.publish`, `*.duplicate`, `*.attach`, `*.detach`, `*.log-update`,
  `coach.checkin.answer-update`. Build the call, run it with **`--dry-run`**, show the user
  the exact request, and get explicit confirmation before sending for real.
- **`*.notice` / `*.notice-id` / `plan.meals.notice` SEND EMAILS** to clients. Treat them
  as send actions — never fire without clear confirmation.
- **`*.delete` is destructive.** Confirm explicitly, and prefer reading the target first
  (a `*.list`/`*.details` call) to verify you have the right id.

When in doubt, dry-run it and ask. A wrong production write is far costlier than a question.

## The loop, end to end

1. `gfit-cli auth.status` → ensure you're logged in (log in if not).
2. `gfit-cli` (browse groups) or `gfit-cli <cmd> -h` (one command's params) to find the call.
3. Build it with documented + passthrough flags.
4. **For any write: `--dry-run` first**, show the request, get confirmation.
5. Run with `--raw`.
6. Check `code == 1`, read `data`; on `code != 1` surface the message.

## Worked examples

See [`references/recipes.md`](references/recipes.md) for end-to-end recipes, including a
**weekly client check-in review** (find who hasn't submitted their check-in and draft
follow-up emails), looking up a client and reading their data, pulling a metric log range,
and safely performing a write. Start there when a task maps to a known workflow.
