# gfit-cli reference (for the weekly check-in skill)

Everything here is what the skill needs to drive `gfit-cli`. The CLI's own `README`
(<https://github.com/dickwu/gfit-cli>) is the full source of truth; this file is the
subset that matters for the check-in workflow.

## Install

`gfit-cli` is a single static binary — no package manager needed.

```bash
# macOS (Apple Silicon)
curl -fsSL -o gfit-cli https://github.com/dickwu/gfit-cli/releases/latest/download/gfit-cli-aarch64-apple-darwin
chmod +x gfit-cli && sudo mv gfit-cli /usr/local/bin/
gfit-cli --version
```

Other targets: `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`. Keep it current with `gfit-cli self.update`.

## Auth

```bash
gfit-cli auth.status              # who am I / is a token saved?
gfit-cli auth.login               # browser sign-in (no password in shell history)
gfit-cli auth.login --email you@example.com --password 'secret'   # headless/scripted
```

The token is saved to `~/.config/gfit.json`. **Every** API command needs a saved token —
with none, only `auth.login`, `auth.status`, `-h`, and `--version` run, and everything
else exits with `not logged in`. The coach must be logged in **as the coach whose clients
you want** — `coach.clients` returns *that* logged-in coach's assigned clients.

## Output format — always use `--raw`

By default the CLI pretty-prints. For parsing, pass `--raw` to get compact JSON of the API
response. The response is an envelope: **success is `code == 1`**, and the useful payload
is under `data`. If `code != 1`, surface the message and stop — don't invent data.

```bash
gfit-cli coach.clients --raw
# {"code":1,"data":[ ... clients ... ], ...}
```

If you ever want to see the exact request a command will send without hitting the network,
add `--dry-run` (works without a token):

```bash
$ gfit-cli coach.checkin.list --id 42 --dry-run
POST https://api.gfitwellness.ca/staff/member/checkin
auth: Authorization: <token>
body: { "id": "42" }
```

## The two commands this skill uses

### `coach.clients` — the roster

```bash
gfit-cli coach.clients --raw
gfit-cli coach.clients --search 'john' --raw   # optional client-side name/email filter
```

`POST staff/workout/client/list`. Returns the logged-in coach's assigned clients. Each
client object carries (names may differ slightly in the live payload — match by meaning):

- an **id** (numeric user id) — you'll feed this to `coach.checkin.list`
- **name** fields (e.g. `first_name` / `last_name`, or a combined `name`/`nickname`)
- **email**
- possibly a **status** (active / inactive / cancelled / paused)
- possibly a **last check-in** or **last activity** date

**Always read the actual JSON before assuming field names.** If a "last check-in" field is
present and trustworthy, you can classify straight from the roster and skip the per-client
calls below.

### `coach.checkin.list` — a member's recent check-ins

```bash
gfit-cli coach.checkin.list --id <client_id> --raw
```

`POST staff/member/checkin`. `--id` is the client's user id (string). Returns that
member's recent check-ins; each entry has a date/timestamp. Take the **most recent** one to
get "last check-in date." An empty list = never checked in.

> Related (not needed for the core flow, but handy): `coach.checkin.note --id <checkin_id>
> --note '...'` adds a private note to a specific check-in. Don't use it as part of the
> email sweep unless the coach asks.

## Scheduling (Tuesday 8:00 AM Central)

The skill itself is on-demand; pick one way to trigger it weekly:

- **By hand (simplest):** every Tuesday morning, tell Claude "run the weekly check-in
  review." Zero setup.
- **Claude Code scheduled routine / cron:** if the coach runs this in Claude Code, set up a
  recurring schedule (cron `0 8 * * 2` in `America/Chicago`) whose prompt is "run the gfit
  weekly check-in review and draft follow-ups." Use Claude Code's scheduling/routines
  feature (e.g. the `/schedule` command) to create it.
- **OS cron + headless:** advanced — a cron job that runs the agent non-interactively. Only
  worthwhile if the coach wants it fully hands-off; the drafts still wait for manual send.

Whatever the trigger, the safety contract is unchanged: **drafts only, never auto-send.**

## Troubleshooting

- `not logged in` → run `gfit-cli auth.login`.
- `code != 1` in a response → the API rejected the call; show the message, don't fabricate.
- Empty roster → confirm the coach is logged in as the right account (`gfit-cli auth.status`).
- A staging/other environment → `GFIT_API_URL` overrides the base URL; `GFIT_CONFIG`
  overrides the token-file path. You rarely need these.
