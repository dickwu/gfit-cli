# gfit-cli command map

You never need this memorised — `gfit-cli` (no args) prints the full grouped list of all
144 commands, and `gfit-cli <command> -h` documents any one of them. This file is the
mental map so you know *where to look* and what each area covers.

## How to navigate

```bash
gfit-cli                         # browse: every command, grouped, one-line each
gfit-cli <command> -h            # drill in: endpoint, auth, parameters (type + meaning), example
gfit-cli <command> --dry-run     # see the exact request a call would send (no token needed)
```

Pick the group from the task, scan its commands in the full list, then `-h` the candidate.

## Groups

### `auth.*` — sign-in & identity
`auth.login` (browser, or `--email/--password`), `auth.logout`, `auth.info` (account info
from the API), `auth.status` / `whoami` (local: token presence, config path, API URL).

### `self.*` — the tool itself
`self.update` (`--check` / `--force` / `--insecure`) upgrades the binary from GitHub
Releases. Local only; no login.

### `admin.*` — admin-tool surface (broad, powerful)
The widest group. Highlights:
- **Clients/users:** `admin.home`, `admin.clients`, `admin.users` (`--type client|workout|all`,
  alias `admin.members`), `admin.client-create`, `admin.client-update`,
  `admin.client-coach-update`, `admin.user-plans`, `admin.user-change-workout`.
- **Coaches/config:** `admin.coaches`, `admin.coach-clients`, `admin.config`.
- **Nutrition:** `admin.meal.*` (list/generate/update/update-targets/duplicate/delete/
  preview/publish/create), `admin.food.*` (list/search/categories/create/update/delete),
  `admin.food-split.*`, `admin.lever.*`.
- **Workouts:** `admin.workout.*` (list/generate/publish/notice/notice-id/videos/video-plan/
  update/duplicate/delete), `admin.video.*`, `admin.split.*` (incl. `report`,
  `details-*`, `attach`/`detach`).
- **Files & knowledge:** `admin.upload` (multipart: `--file <path> --type <type>`),
  `admin.nowledge.*` (status/documents/sources/search/answer/create/update/delete/…).

### `coach.*` — the coaching views (maps to `staff/*`)
What a coach uses day to day. This is usually the right group for "my clients" tasks:
- **Roster & profile:** `coach.clients` (`--search`, client-side filter), `coach.search`,
  `coach.info`, `coach.create`, `coach.client-update`, `coach.attach`/`coach.detach`,
  `coach.publish-date`, `coach.profile-history`, `coach.merge-user`, `coach.email.*`.
- **Check-ins:** `coach.checkin.list` (`--id <user>`), `coach.checkin.note`,
  `coach.checkin.answer-update`.
- **Metrics (date-range reads + updates):** `coach.weight.*`, `coach.step.*`,
  `coach.sleep.*` (each: `log-list` / `log-update` / `log-delete`, plus `weight.info`),
  `coach.text.*` (`stress` / `dietary` / `energy` reads, `update`).
- **Plans:** `coach.meal.*`, `coach.food-split.*`, `coach.split.*`, `coach.workout.*`
  (list/generate/publish/notice/duplicate/delete/…).
- **Invoices:** `coach.invoice.list`, `coach.invoice.download`, `coach.invoice.dietitian`.

### `plan.*` — member-uploaded meal plans
`plan.meals` (list), `plan.meals.update`, `plan.meals.notice` (sends an email).

## `admin.*` vs `coach.*`

Both can touch client data; they hit different API surfaces and scopes. As a coach, prefer
`coach.*` — it's scoped to *your* assigned clients and matches the app's coach views. Use
`admin.*` only for admin-tool operations that `coach.*` doesn't expose. When two commands
look similar (e.g. `admin.meal.list` vs `coach.meal.list`), pick the one matching the role
the user is acting in, and `-h` both if unsure.

## Reminders that apply everywhere

- Success = `code == 1`; payload in `data`. Use `--raw` to parse.
- `--dry-run` previews any request without a token — use it before every write.
- `*.notice*` and `plan.meals.notice` **send email**; `*.delete` is destructive. Confirm first.
