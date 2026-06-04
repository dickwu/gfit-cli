# GFit (gfit-cli extension) — how to use it

You have the **gfit-cli** MCP extension: it drives the GFIT wellness API
(api.gfitwellness.ca) for coaching/admin data — clients, check-ins, meal &
workout plans, nutrition, foods, metrics (weight/step/sleep/text logs),
invoices, Nowledge. Tools: gfit_list_commands, gfit_help, gfit_auth_status,
gfit_login, gfit_run. Reach for them whenever I describe a GFit operation even
if I don't say "gfit-cli" (e.g. "pull my clients", "who hasn't checked in this
week", "list weight logs for user 42", "publish that meal plan").

## Golden rule: discover, don't guess
Don't memorize commands — two tools answer almost everything, use them first:
- gfit_list_commands → the full grouped list (~144 commands).
- gfit_help { command: "coach.checkin.list" } → that command's endpoint, auth,
  and every parameter (type + meaning). This is the source of truth; check it
  before calling anything you haven't used.

Commands are dot-named group.action: auth.* (login/status), admin.* (clients,
users, meals, foods, splits, workouts, Nowledge), coach.* (assigned clients,
check-ins, plans, metrics, invoices, emails), plan.* (member meal plans +
notices), self.* (local).

## Auth
Call gfit_auth_status to check if I'm logged in. If not, you don't have my
password — call gfit_login (or just attempt the task; a real call auto-opens
the sign-in page in MY browser). I sign in once; the token is saved and reused.
I act as whoever is logged in (e.g. coach.clients = that coach's clients).

## Running commands
gfit_run { command: "<group.action>", params: { ...key/value... } }. Arbitrary
params are allowed and auto-typed. Output is the API envelope: success = code
== 1 with the payload under data. If code != 1, surface the msg and stop —
never invent data.

## Safe writes (important)
Reads are safe — run freely: *.list, *.search, *.info, *.status, *.details,
coach.checkin.list, metric *.log-list.

Writes/destructive commands (*.create, *.update, *.delete, *.generate,
*.publish, *.notice, …) are NOT executed on the first call — the extension
returns a --dry-run PREVIEW of the exact request instead. Workflow:
1. Call gfit_run for the write → you get a PREVIEW (nothing was sent).
2. Show me that request and ask for confirmation.
3. Only after I say yes, call gfit_run again with the same command/params PLUS
   confirm: true.
Extra care: *.notice / plan.meals.notice SEND EMAILS to clients; *.delete
DESTROYS data — for deletes, read the target first (a *.list/*.details) to
verify the id. When in doubt, preview and ask.

## The loop
auth_status → discover (gfit_list_commands / gfit_help) → build gfit_run → for
writes, preview + confirm → read code/data.

## Recipe: weekly check-in review (reads only, safe)
Find which of my clients haven't submitted their weekly check-in and draft
follow-ups as Gmail drafts (never sent):
1. gfit_run { command: "coach.clients" } → roster; per client get id, name,
   email, status. Skip inactive/cancelled or no-email.
2. Per active client: gfit_run { command: "coach.checkin.list", params: { id:
   <id> } } → most recent check-in date (empty = never).
3. Classify by days since (my timezone): ≤7 up to date (skip); 8–14 Tier 1
   light nudge; 15–28 Tier 2 caring; >28/never Tier 3 re-engage; brand-new
   (<7d, no check-in) skip.
4. One Gmail DRAFT per client needing follow-up, in my voice, personalized by
   first name, with the link https://www.gfitwellness.ca/weekly-check-in,
   signed by me. Drafts only — I review and send.
5. Report a table: name, email, last check-in, days, tier, draft/skip + reason.
Uses only coach.clients + coach.checkin.list (reads) + Gmail drafts — no GFit
writes, safe any time.
