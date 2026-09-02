# GitHub Copilot

Burnrate tracks GitHub Copilot **premium requests** against your plan's
monthly allowance. The allowance resets on the **1st of each month at
00:00 UTC**.

## Setup

Add it via **Preferences → Add account → GitHub Copilot**. A GitHub token is
**required** — see [Data source](#data-source).

Pick your **plan** so usage becomes a quota with warning/critical status:

| Plan       | Premium requests / month |
| ---------- | ------------------------ |
| Free       | 50                       |
| Pro        | 300                      |
| Pro+       | 1,500                    |
| Business   | 300                      |
| Enterprise | 1,000                    |
| Custom     | your own limit           |

Without a plan, Burnrate shows usage without a limit.

## Data source

**GitHub billing API — required.** Enter a GitHub **personal access token
(classic)** in the account form and Burnrate fetches your exact month-to-date
premium request usage from GitHub's billing API
(`GET /users/{username}/settings/billing/premium_request/usage`).

Upstream Burnrate can fall back to a lower-bound estimate counted from local
Copilot CLI session logs. **This fork removes all local session scanning**, so
there is no fallback: without a working token the account shows an error card
asking you to configure one.

## Caveats

- GitHub's billing endpoints do **not** support fine-grained tokens — it must
  be a classic PAT.
- They report only usage billed to your **personal** account. A license
  managed by an organization or enterprise is not included.
- A revoked token or a network failure produces an error card, not a degraded
  estimate.
