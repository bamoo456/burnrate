# Codex

Burnrate reads Codex **Pro/Max plan** info and **rate-limit buckets**
(5-hour and weekly windows, including Spark buckets where present) directly
from the Codex app server over stdio — the same data the Codex UI shows.

Business/usage-based workspaces that meter **credits** instead of time windows
show a **Monthly credits** allowance (used / limit and reset from the
workspace spend controls). Once Burnrate has collected samples over time it
also estimates a **burn rate** and runway from observed credit consumption.
The Codex API only reports point-in-time values, so a fresh install shows no
estimate for the first hours.

## Requirements

- The `codex` CLI installed and signed in.

Accounts are added explicitly (this fork does not auto-detect them). The
account email comes from the local `auth.json` identity token.

## Multiple accounts

Additional accounts sign in from **Preferences → Add account → Sign in with
browser**. Burnrate runs `codex login` under an isolated per-account
`CODEX_HOME`, so secondary accounts never disturb your terminal session in
`~/.codex`.

## Notes

- If the `codex` binary lives somewhere unusual, set `BURNRATE_CODEX_BIN`.
- Inherited credential env vars (e.g. `OPENAI_API_KEY`) are stripped from
  spawned CLIs so they can't shadow the signed-in account.
