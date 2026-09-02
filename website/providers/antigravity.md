# Antigravity

Burnrate shows Google Antigravity's **weekly** and **five-hour** quota for both
model pools — Gemini, and Claude + GPT — plus your account email and plan.

## Setup

There is **no API key to enter**: Antigravity's credentials belong to its CLI.

1. Install the CLI: `brew install --cask antigravity-cli`.
2. Run `agy` once in a terminal and sign in.
3. In **Preferences → Add account → Antigravity**, set a label and add it.

If `agy` is not on your `PATH`, point Burnrate at it with `BURNRATE_AGY_BIN`
(or `ANTIGRAVITY_CLI_PATH`).

## What it shows

| Bucket          | Label               | Dashboard column |
| --------------- | ------------------- | ---------------- |
| `gemini-5h`     | 5-hour              | 5-hour           |
| `gemini-weekly` | Weekly              | Weekly           |
| `3p-5h`         | Claude + GPT 5-hour | (extra)          |
| `3p-weekly`     | Claude + GPT Weekly | (extra)          |

Antigravity reports a remaining _fraction_, not a token or request count, so
Burnrate models each bucket as a percentage against a 100% limit. The Gemini
pool takes the two dashboard columns; the Claude + GPT pool renders as extra
rows on the card. There is no monthly window, so that column stays empty.

## How it works

Antigravity publishes no quota API. The `agy` CLI embeds the same Connect-RPC
language server the desktop app uses, on a loopback HTTPS port, and answers
`RetrieveUserQuotaSummary` with exactly the two groups the IDE's Model Quota
UI shows. `GetUserStatus` supplies the email and plan.

`agy` is an interactive TUI: it exits immediately without a controlling
terminal, and serves quota only while running. So a refresh reuses an `agy` you
already have running, or starts one under a PTY, waits for a quota endpoint to
answer, reads it, and shuts it back down. Burnrate never parses terminal
output, and never signals an `agy` you started yourself.

## Caveats

- **Single account.** Quota comes from whichever account `agy` is signed in as
  (`~/.gemini/google_accounts.json` holds one active account). Burnrate checks
  the reported email against the account's own, so a second Antigravity
  account shows an error naming both addresses rather than repeating the
  signed-in account's numbers. To follow a different account, switch it in
  `agy`.
- A cold `agy` needs a few seconds of keyring authentication before its quota
  endpoints answer, so the first refresh after a reboot can be slower.
- If `agy` is signed out, the account shows an error asking you to run `agy`
  in a terminal and sign in.
