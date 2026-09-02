# Getting Started

## First launch

Burnrate starts with no accounts. It never scans your filesystem to discover
them, so every account is added explicitly — see [Adding accounts](#adding-accounts).

The tray icon appears in the menu bar; left-click for the usage popover,
right-click for Preferences, Refresh, and Quit.

## Adding accounts

Open **Preferences** (tray right-click, or the gear in the popover header)
and use **Add account**:

- **Sign in with browser** (Claude Code, Codex) — Burnrate shells out to the
  official `claude` / `codex` CLI, opens your browser, and reads back the
  resulting email and usage. For Claude Code, copy the authentication code
  shown in the browser at the end and paste it into the login dialog.
- **Manual token** (Claude Code, Codex) — paste an existing OAuth token
  instead of signing in.
- **API key** (OpenRouter, Runpod, OpenCode Go) — paste the provider API key.
- **GitHub Copilot** — pick your plan and paste a GitHub personal access token
  (classic). See the [Copilot provider page](/providers/github-copilot).
- **Antigravity** — no key to enter; sign in once with the `agy` CLI and
  Burnrate reads your quota from it. See the
  [Antigravity provider page](/providers/antigravity).
- **AWS** — pick a credential profile (or leave blank for the default
  chain). See the [AWS provider page](/providers/aws).

### Multiple accounts

Claude Code and Codex accounts added through browser sign-in each get their
own isolated CLI config dir under `<config-dir>/cli/<provider>/<id>`, so they
never disturb the shared terminal session in `~/.claude` / `~/.codex`.
Signing in the same email twice refreshes the existing account instead of
creating a duplicate.

Antigravity is the exception: its quota comes from whichever account the `agy`
CLI is currently signed in as, so it is effectively single-account.

### Re-authenticating

**Sign in again** (in an account's edit panel) re-authenticates that account
in place, in its own isolated CLI dir. It is the fix for a stale or expired
login. Burnrate never re-authenticates your system-default `~/.claude` /
`~/.codex` session — that belongs to your own terminal.

### Removing accounts

**Sign out** (or removing the account) clears only that managed account —
running the CLI sign-out and deleting its isolated dir. It never touches
your system session.

## Reordering

Drag account cards to reorder them — in the tray popover or the Preferences
list. The order persists and is shared by both windows.
