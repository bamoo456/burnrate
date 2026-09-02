# OpenCode Go

Burnrate shows your **rolling**, **weekly**, and **monthly** usage as
percentages of the plan limit, each with its reset time, from the OpenCode Zen
usage API.

## Setup

1. Get your Zen API key from your OpenCode account.
2. In **Preferences → Add account → OpenCode Go**, paste the key.

The key is stored in the OS keyring by default (plaintext storage is an
explicit opt-in per account).

## What it shows

| Bucket    | Dashboard column |
| --------- | ---------------- |
| `rolling` | 5-hour           |
| `weekly`  | Weekly           |
| `monthly` | Monthly          |

Each bucket reports percent used against a 100% limit, so the standard
warning (≤20% remaining) and critical (≤5%) thresholds apply.

## Notes

- The balance is not exposed by the API; Burnrate says so explicitly rather
  than implying it is zero.
- `BURNRATE_OPENCODE_GO_USAGE_URL` overrides the usage endpoint (for a proxy
  or a test server). Custom endpoints must be HTTPS, localhost excepted.
