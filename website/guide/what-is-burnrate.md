# What is Burnrate?

Burnrate is a desktop usage monitor for AI provider quotas, credits, spend,
and subscription limits. It lives in the system tray (the menu bar on macOS)
and keeps live meters for every account you add:

- **Claude Code** — subscription buckets (5-hour, weekly, model-specific)
  with stale-auth checks via `claude auth status`.
- **Codex** — Pro/Max plan and rate-limit buckets read from the Codex app
  server.
- **GitHub Copilot** — premium requests against your plan's monthly
  allowance, read from the GitHub billing API.
- **Antigravity** — weekly and five-hour quota for both model pools (Gemini,
  Claude + GPT), read from the `agy` CLI's local quota server.
- **OpenCode Go** — rolling, weekly, and monthly usage from your Zen API key.
- **OpenRouter** — remaining prepaid credits.
- **Runpod** — prepaid balance, current burn, runway, and active resources.
- **AWS** — Cost Explorer month-to-date spend with optional budgets and
  per-service buckets.

Built with Tauri 2 — a Rust backend and a React/TypeScript UI — and shipped
as native bundles for macOS, Linux, and Windows.

## How it works

- A left-click on the tray icon opens a translucent popover with usage cards
  for every enabled account; right-click offers Preferences, Refresh, and
  Quit.
- Burnrate refreshes in the background every five minutes and caches
  successful snapshots for five minutes to avoid tight polling.
- **Every account is added explicitly** — Burnrate never scans your
  filesystem to discover accounts. Claude Code and Codex sign in from the app
  via browser OAuth; OpenRouter, Runpod, and OpenCode Go take an API key;
  Copilot takes a GitHub token; AWS uses your existing credential chain; and
  Antigravity reads quota from the `agy` CLI you are already signed in to.
- Statuses roll up: the tray icon summarizes all accounts, flagging the one
  closest to exhaustion (warning at ≤20% remaining, exhausted at ≤5%).
- It hides from the Dock by default and appears only while Preferences is
  open.

## Next steps

- [Install Burnrate](/guide/installation)
- [Add your accounts](/guide/getting-started)
- [Provider specifics](/providers/claude-code)
