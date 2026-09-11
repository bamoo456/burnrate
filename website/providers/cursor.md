# Cursor

Burnrate shows the **included usage** pool for your Cursor plan — spend so far
this billing period, the pool's total, and the date it resets.

## Setup

There is **no API key to enter**: Cursor's credentials belong to its CLI.

1. Install the CLI: `curl https://cursor.com/install -fsS | bash`.
2. Run `cursor-agent login` once and sign in.
3. In **Preferences → Add account → Cursor**, set a label and add it.

macOS only — the credential lives in the login Keychain.

## What it shows

| Bucket | Label   | Dashboard column |
| ------ | ------- | ---------------- |
| `plan` | Monthly | Monthly          |

Included usage is a **USD pool**, so the bucket is in dollars: used, total, and
remaining, resetting on your billing date (not the 1st of the month). Seats
whose pool the server does not disclose — some team seats — report only a
percentage, and Burnrate falls back to a `%` bucket against a 100% limit rather
than showing nothing.

## How it works

Cursor publishes no per-user quota API: `api.cursor.com` is scoped to team
admins. Burnrate calls the same Connect-RPC endpoint the IDE dashboard uses,
`aiserver.v1.DashboardService/GetCurrentPeriodUsage`, with the access token
`cursor-agent` stores in the Keychain (service `cursor-access-token`).

Burnrate only ever **reads** that credential. The paired `cursor-refresh-token`
belongs to the CLI, and a second writer rotating it would sign your terminal
sessions out.

## Caveats

- **Unofficial endpoint.** It backs Cursor's own dashboard rather than a
  published API, so its shape can change without notice.
- **Single account.** Usage comes from whichever account `cursor-agent` is
  signed in as. To follow a different one, switch it in the CLI.
- **No local insights.** claudex indexes no Cursor session source, so Cursor
  appears in quota cards only, not in local usage analytics.
- If the CLI is signed out, the account shows an error asking you to run
  `cursor-agent login`.
