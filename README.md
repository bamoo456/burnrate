# Burnrate

[![CI](https://github.com/bamoo456/burnrate/actions/workflows/ci.yml/badge.svg)](https://github.com/bamoo456/burnrate/actions/workflows/ci.yml)
[![Release](https://github.com/bamoo456/burnrate/actions/workflows/release.yml/badge.svg)](https://github.com/bamoo456/burnrate/actions/workflows/release.yml)
[![Docs](https://github.com/bamoo456/burnrate/actions/workflows/docs.yml/badge.svg)](https://bamoo456.github.io/burnrate/)
[![latest release](https://img.shields.io/github/v/release/bamoo456/burnrate?sort=semver)](https://github.com/bamoo456/burnrate/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)](https://github.com/bamoo456/burnrate/releases/latest)

Desktop usage monitor for Claude Code, Codex, GitHub Copilot, Antigravity, OpenCode Go, OpenRouter, Runpod, and AWS quotas, credits, spend, and subscription limits. Built with Tauri 2 (Rust + React/TypeScript) and lives in the system tray (the menu bar on macOS).

> **Fork notice.** This is a fork of [jamesbrink/burnrate](https://github.com/jamesbrink/burnrate) that adds the Antigravity provider and deliberately removes all local-filesystem session scanning: accounts are added explicitly instead of auto-detected, and claudex-backed local usage insights are disabled. See [Differences from upstream](#differences-from-upstream).

**Documentation: [bamoo456.github.io/burnrate](https://bamoo456.github.io/burnrate/)**

## Screenshots

<p align="center">
  <img src="website/public/screenshots/preferences.png" alt="Burnrate Preferences window showing per-account usage across Claude Code, Codex, Runpod, and OpenRouter" width="62%" />
  &nbsp;
  <img src="website/public/screenshots/tray.png" alt="Burnrate menu-bar popover with live quota meters for each account" width="30%" />
</p>

<p align="center"><em>The full Preferences window (left) and the menu-bar popover (right).</em></p>

## Features

- Menu-bar tray summary with a left-click usage popover and right-click actions (Preferences, Refresh, Quit).
- Native translucent (vibrancy) popover on macOS that follows the system light/dark appearance, sizes itself to its content, and dismisses when it loses focus.
- Native Preferences window for account management and provider setup.
- **Accounts are added explicitly** — Burnrate never scans your filesystem to discover them. Claude Code and Codex sign in from the app via browser OAuth; OpenRouter, Runpod, and OpenCode Go take an API key; AWS uses your existing profile/default credential chain; Antigravity reads quota through the `agy` CLI you are already signed in to.
- **Multiple Claude Code and Codex accounts**, each signed in from the app via browser OAuth and shown with its email address and usage.
- **Drag to reorder** accounts — reorder the tray usage cards or the Preferences list; the order persists across both windows.
- Claude Code subscription buckets (5-hour, weekly, model-specific) with stale-auth checks via `claude auth status`.
- Codex Pro/Max plan and rate-limit buckets read from the Codex app server.
- **GitHub Copilot premium requests** per month against your plan's allowance, read from the billing API. Requires a GitHub token (a classic PAT — the billing endpoints reject fine-grained tokens).
- **Antigravity** weekly and five-hour quota for both model pools (Gemini, Claude + GPT), plus account email and plan, read from the `agy` CLI's local quota server.
- **OpenCode Go** rolling, weekly, and monthly usage from your Zen API key.
- Runpod prepaid balance, current spend, burn-rate runway, active resources, and recent Pods/Serverless/storage costs.
- AWS Cost Explorer month-to-date USD spend with optional monthly budgets and configurable service/tag/cost-category buckets such as Bedrock, EC2 compute, and S3.
- Secrets in the OS keyring by default, with an explicit plaintext fallback.
- Hides from the Dock by default; appears only while Preferences is open.
- **Automatic updates (macOS)** with selectable **Stable** and **Nightly** channels: a dismissible banner and tray "Check for Updates…" entry offer a signature-verified one-click "Install & Restart." Choose the channel under Preferences → Updates.

## Differences from upstream

This fork removes every local-filesystem session-scanning path from
[jamesbrink/burnrate](https://github.com/jamesbrink/burnrate) and adds one provider.

|                          | Upstream                                                         | This fork                                                                                             |
| ------------------------ | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Account discovery        | Auto-detects Claude Code / Codex / Copilot from local config     | **Explicit only** — `detect_accounts()` returns nothing (`src/providers/mod.rs`)                      |
| Local usage insights     | claudex-backed daily cost, projections, model/project breakdowns | **Disabled** — `local_session_scanning_disabled()` is always true (`src/insights.rs`)                 |
| Copilot premium requests | Local estimate from CLI sessions, GitHub token optional          | **Token required** — the local estimate always errors (`src/insights.rs`, `src/providers/copilot.rs`) |
| Antigravity              | —                                                                | **Added** — quota via the `agy` CLI's local server (`src/providers/antigravity.rs`)                   |
| crates.io                | Publishes `burnrate`                                             | Not published; GitHub Releases only                                                                   |

The upstream docs site still documents the upstream behavior, so prefer the
[docs for this fork](https://bamoo456.github.io/burnrate/).

## Install

Download the native bundle for your platform from [GitHub Releases](https://github.com/bamoo456/burnrate/releases).

This fork is **not published to crates.io** — the `burnrate` crate name belongs to upstream — so `cargo install burnrate` gets you the upstream build, not this one. To build from source instead:

```sh
git clone https://github.com/bamoo456/burnrate && cd burnrate
npm install && ./scripts/package-dmg    # macOS .app + .dmg
```

See the [installation guide](https://bamoo456.github.io/burnrate/guide/installation) for Nix and signed macOS builds.

## Documentation

Setup, provider specifics (including AWS permissions), configuration, and troubleshooting live on the docs site:

- [Getting started](https://bamoo456.github.io/burnrate/guide/getting-started) — accounts, browser sign-in, multi-account isolation
- [Configuration](https://bamoo456.github.io/burnrate/guide/configuration) — storage paths, secrets, environment variables
- [Providers](https://bamoo456.github.io/burnrate/providers/claude-code) — Claude Code, Codex, GitHub Copilot, Antigravity, OpenCode Go, OpenRouter, Runpod, AWS
- [Troubleshooting](https://bamoo456.github.io/burnrate/guide/troubleshooting) — keychain prompts, CLI discovery, stale auth

## Development

```sh
npm install
npm run dev      # tauri dev — launches the desktop app + tray
```

A Nix devshell exposes the full workflow (`nix develop`, then `dev`, `check`, `test`, `fmt`, `build-app`, `build-pure`, `package-dmg`, `docs-dev`). See [AGENTS.md](AGENTS.md) for the architecture map and complete command reference.

The docs site is a VitePress app in `website/` — `docs-dev` starts it with hot reload; pushes to `main` deploy it to GitHub Pages.

## Releases

- `release-plz` manages release PRs, CHANGELOG, and version tags. crates.io publishing is **off** (`release-plz.toml` `publish = false`).
- Tagging `v*` builds native Tauri bundles (macOS, Linux, Windows) and uploads them with checksums to the GitHub Release, then publishes a signed `latest.json` (the **Stable** auto-update manifest).
- A `nightly` workflow runs after green `CI` on `main`, building a signed macOS pre-release and promoting it to the rolling `nightly` release/manifest (the **Nightly** channel).
- Auto-update signing uses a Tauri minisign keypair: the public key lives in `tauri.conf.json`; the private key + passphrase are the `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repo secrets.

## License

[MIT](LICENSE)
