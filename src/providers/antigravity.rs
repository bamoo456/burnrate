//! Google Antigravity quota via the `agy` CLI's local quota server.
//!
//! Antigravity publishes no public quota API — the plan docs only point at the
//! in-app settings page. The `agy` CLI, however, embeds the same Connect-RPC
//! language server the desktop app uses, on an ephemeral localhost HTTPS port,
//! and it answers `RetrieveUserQuotaSummary` with exactly the two groups the
//! IDE's Model Quota UI shows (Gemini, Claude + GPT), each with a weekly and a
//! five-hour bucket.
//!
//! `agy` is a Bubble Tea TUI that exits immediately without a controlling
//! terminal, and it serves quota only while that interactive process lives. So
//! a fetch spawns it under a PTY (`script -q /dev/null agy`), waits for a quota
//! endpoint to actually answer (a fresh process binds its port well before the
//! service is ready), reads the payload, and tears the process down again. An
//! `agy` the user already has running is reused instead — and never killed.
//!
//! Scope: the CLI source only. The desktop app's `language_server` probe needs
//! a `--csrf_token` scraped from process arguments, the IDE's local server 404s
//! on `RetrieveUserQuotaSummary`, and the Google OAuth path needs a client
//! secret extracted from the app bundle. All three are deliberate follow-ups.

use std::{
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;
use tokio::{process::Command, time::timeout};

use crate::models::{
    AccountConfig, SubscriptionPlan, SubscriptionSnapshot, UsageBucketSnapshot, UsageSnapshot,
};

use super::{overall_status, primary_quota, status_from_remaining};

/// Connect-RPC service prefix shared by the desktop app, the IDE extension, and
/// the CLI's embedded server.
const RPC_PREFIX: &str = "exa.language_server_pb.LanguageServerService";

/// Minimal client metadata the language server expects on every call.
const RPC_BODY: &str = r#"{"metadata":{"ideName":"antigravity","extensionName":"antigravity","locale":"en","ideVersion":"unknown"}}"#;

/// A cold `agy` needs several seconds of keyring auth before its quota
/// endpoints answer, so readiness is polled rather than assumed.
const READY_DEADLINE: Duration = Duration::from_secs(35);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(750);
/// Per-request budget once a port is known.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
/// Reusing an already-running `agy` must not stall a refresh.
const REUSE_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) fn agy_binary() -> String {
    for key in ["BURNRATE_AGY_BIN", "ANTIGRAVITY_CLI_PATH"] {
        if let Some(value) = std::env::var_os(key) {
            let value = value.to_string_lossy().trim().to_string();
            if !value.is_empty() {
                return value;
            }
        }
    }
    "agy".to_string()
}

pub(crate) async fn fetch(account: &AccountConfig) -> Result<UsageSnapshot> {
    let http = local_client()?;

    // Prefer an `agy` the user is already running: no spawn, no teardown.
    if let Some(port) = reuse_running_agy(&http).await {
        return collect(&http, account, port).await;
    }

    let mut session = AgySession::spawn().await?;
    let port = session.wait_for_ready(&http).await?;
    let snapshot = collect(&http, account, port).await;
    session.shutdown().await;
    snapshot
}

/// HTTPS client for the language server's **loopback-only** endpoint. The CLI
/// serves a self-signed certificate, so verification is disabled — acceptable
/// only because every request this client makes targets `127.0.0.1`. Never
/// reuse it for a remote host.
fn local_client() -> Result<Client> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(PROBE_TIMEOUT)
        .build()
        .context("failed to build the Antigravity local HTTPS client")
}

fn rpc_url(port: u16, method: &str) -> String {
    format!("https://127.0.0.1:{port}/{RPC_PREFIX}/{method}")
}

async fn rpc(http: &Client, port: u16, method: &str) -> Result<serde_json::Value> {
    let response = http
        .post(rpc_url(port, method))
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .body(RPC_BODY)
        .send()
        .await
        .with_context(|| format!("Antigravity {method} request failed"))?
        .error_for_status()
        .with_context(|| format!("Antigravity {method} returned an error status"))?;
    response
        .json()
        .await
        .with_context(|| format!("failed to decode the Antigravity {method} response"))
}

/// Fetch the quota summary, then enrich it with identity from `GetUserStatus`.
/// The summary is the only required call: identity is nice-to-have context, so
/// a `GetUserStatus` failure must not lose real quota numbers.
async fn collect(http: &Client, account: &AccountConfig, port: u16) -> Result<UsageSnapshot> {
    let summary = rpc(http, port, "RetrieveUserQuotaSummary").await?;
    let status = rpc(http, port, "GetUserStatus").await.ok();
    parse_snapshot(account, &summary, status.as_ref())
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

pub(crate) fn parse_snapshot(
    account: &AccountConfig,
    summary: &serde_json::Value,
    user_status: Option<&serde_json::Value>,
) -> Result<UsageSnapshot> {
    let groups = summary
        .pointer("/response/groups")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow!("Antigravity quota summary did not include any groups"))?;

    let mut buckets: Vec<UsageBucketSnapshot> = Vec::new();
    for group in groups {
        let group_name = group
            .get("displayName")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Some(entries) = group.get("buckets").and_then(serde_json::Value::as_array) else {
            continue;
        };
        for entry in entries {
            if let Some(bucket) = parse_bucket(group_name, entry) {
                buckets.push(bucket);
            }
        }
    }

    if buckets.is_empty() {
        return Err(anyhow!(
            "Antigravity quota summary contained no usable buckets"
        ));
    }
    // Gemini before Claude + GPT, five-hour before weekly, so the dashboard's
    // primary quota is the most immediate window of the main model family.
    buckets.sort_by_key(|bucket| bucket_rank(&bucket.id));

    Ok(UsageSnapshot {
        account_id: account.id.clone(),
        provider: account.provider,
        label: account.label.clone(),
        status: overall_status(&buckets),
        email: user_status.and_then(account_email),
        subscription: user_status.and_then(subscription),
        quota: primary_quota(&buckets),
        usage_buckets: buckets,
        message: None,
        fetched_at: Utc::now(),
    })
}

/// Antigravity reports a remaining *fraction*; there is no token or request
/// count to show. Model it as a percentage so the existing meters, thresholds,
/// and dashboard columns work unchanged.
fn parse_bucket(group_name: &str, entry: &serde_json::Value) -> Option<UsageBucketSnapshot> {
    let id = entry.get("bucketId").and_then(serde_json::Value::as_str)?;
    let fraction = entry
        .get("remainingFraction")
        .and_then(serde_json::Value::as_f64)?
        .clamp(0.0, 1.0);
    let remaining = fraction * 100.0;
    let window = normalize_window(entry.get("window").and_then(serde_json::Value::as_str), id)?;

    Some(UsageBucketSnapshot {
        id: id.to_string(),
        label: bucket_label(group_name, id, &window),
        window: Some(window),
        used: 100.0 - remaining,
        limit: Some(100.0),
        remaining: Some(remaining),
        unit: "%".to_string(),
        reset_at: entry
            .get("resetTime")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_reset_time),
        status: status_from_remaining(Some(100.0), Some(remaining)),
    })
}

/// Map Antigravity's `5h` / `weekly` onto the window vocabulary the UI's
/// dashboard columns already understand, falling back to the bucket id when the
/// field is absent.
fn normalize_window(window: Option<&str>, id: &str) -> Option<String> {
    let hint = window.unwrap_or(id).to_ascii_lowercase();
    if hint.contains("5h") || (hint.contains('5') && hint.contains("hour")) {
        Some("5-hour".to_string())
    } else if hint.contains("week") {
        Some("weekly".to_string())
    } else {
        None
    }
}

/// The Gemini pool is the headline quota, so its buckets take the bare window
/// labels that win the dashboard's Weekly / 5-hour columns. Other families keep
/// a qualified label and surface as extras.
fn bucket_label(group_name: &str, id: &str, window: &str) -> String {
    let window_label = if window == "5-hour" {
        "5-hour"
    } else {
        "Weekly"
    };
    if is_primary_family(id) {
        return window_label.to_string();
    }
    let family = family_label(group_name, id);
    format!("{family} {window_label}")
}

fn is_primary_family(id: &str) -> bool {
    id.starts_with("gemini")
}

fn family_label(group_name: &str, id: &str) -> String {
    // "Claude and GPT models" -> "Claude + GPT"; fall back to the bucket id
    // prefix when a future payload adds an unnamed family.
    let trimmed = group_name.trim().trim_end_matches(" models");
    if trimmed.is_empty() {
        return id.split('-').next().unwrap_or(id).to_string();
    }
    trimmed.replace(" and ", " + ")
}

/// Sort key: Gemini first, then five-hour before weekly.
fn bucket_rank(id: &str) -> (u8, u8) {
    let family = if is_primary_family(id) { 0 } else { 1 };
    let window = if id.contains("5h") { 0 } else { 1 };
    (family, window)
}

/// `resetTime` is ISO-8601 in observed payloads; epoch seconds are accepted as
/// the documented legacy shape.
fn parse_reset_time(value: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed.with_timezone(&Utc));
    }
    value
        .parse::<i64>()
        .ok()
        .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
}

fn account_email(user_status: &serde_json::Value) -> Option<String> {
    user_status
        .pointer("/userStatus/email")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(ToString::to_string)
}

fn subscription(user_status: &serde_json::Value) -> Option<SubscriptionSnapshot> {
    let label = [
        "/userStatus/userTier/name",
        "/userStatus/planStatus/planInfo/planName",
    ]
    .iter()
    .find_map(|pointer| {
        user_status
            .pointer(pointer)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })?;

    Some(SubscriptionSnapshot {
        plan: plan_from_label(label),
        plan_label: label.to_string(),
        rate_limit_tier: user_status
            .pointer("/userStatus/planStatus/planInfo/teamsTier")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
        extra_usage_enabled: None,
        source: "antigravity-cli".to_string(),
    })
}

fn plan_from_label(label: &str) -> SubscriptionPlan {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("ultra") {
        SubscriptionPlan::Max
    } else if normalized.contains("enterprise") {
        SubscriptionPlan::Enterprise
    } else if normalized.contains("team") {
        SubscriptionPlan::Team
    } else if normalized.contains("pro") {
        SubscriptionPlan::Pro
    } else if normalized.contains("free") {
        SubscriptionPlan::Free
    } else {
        SubscriptionPlan::Unknown
    }
}

// ---------------------------------------------------------------------------
// `agy` process + port discovery
// ---------------------------------------------------------------------------

/// A Burnrate-owned `agy` run. Dropping it kills both the PTY wrapper and the
/// `agy` child; an externally started `agy` never becomes a session, so it can
/// never be killed here.
struct AgySession {
    wrapper: tokio::process::Child,
    agy_pid: Option<u32>,
}

impl AgySession {
    async fn spawn() -> Result<Self> {
        let binary = super::resolve_cli(&agy_binary());
        if !binary.is_absolute() || !binary.exists() {
            return Err(anyhow!(
                "The Antigravity CLI (`agy`) was not found. Install it (`brew install --cask antigravity-cli`) and run `agy` once to sign in, or set BURNRATE_AGY_BIN."
            ));
        }

        // `agy` is a Bubble Tea TUI: without a controlling terminal it exits with
        // "could not open TTY" before binding a port. `script` supplies the PTY
        // and discards the rendering; no terminal output is ever parsed.
        let mut command = Command::new("/usr/bin/script");
        command
            .arg("-q")
            .arg("/dev/null")
            .arg(&binary)
            .env("PATH", super::augmented_path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        super::strip_credential_env(&mut command);

        let wrapper = command
            .spawn()
            .context("failed to start the Antigravity CLI (`agy`)")?;
        Ok(Self {
            wrapper,
            agy_pid: None,
        })
    }

    /// Poll until a quota endpoint actually answers. A fresh `agy` binds its
    /// port before the quota service finishes authenticating, so an open port is
    /// not readiness.
    async fn wait_for_ready(&mut self, http: &Client) -> Result<u16> {
        let started = Instant::now();
        let mut last_error: Option<String> = None;

        while started.elapsed() < READY_DEADLINE {
            if let Some(pid) = self.resolve_agy_pid().await {
                for port in listening_ports(pid).await {
                    match timeout(PROBE_TIMEOUT, rpc(http, port, "RetrieveUserQuotaSummary")).await
                    {
                        Ok(Ok(value)) if value.pointer("/response/groups").is_some() => {
                            return Ok(port);
                        }
                        Ok(Err(error)) => last_error = Some(error.to_string()),
                        Ok(Ok(_)) => last_error = Some("quota summary was empty".to_string()),
                        Err(_) => last_error = Some("quota probe timed out".to_string()),
                    }
                }
            }
            if let Ok(Some(exit)) = self.wrapper.try_wait() {
                return Err(anyhow!(
                    "The Antigravity CLI (`agy`) exited early ({exit}). Run `agy` in a terminal and sign in, then refresh."
                ));
            }
            tokio::time::sleep(READY_POLL_INTERVAL).await;
        }

        Err(anyhow!(
            "The Antigravity CLI (`agy`) did not report quota within {}s.{}",
            READY_DEADLINE.as_secs(),
            last_error
                .map(|error| format!(" Last error: {error}"))
                .unwrap_or_else(
                    || " Run `agy` in a terminal and sign in, then refresh.".to_string()
                )
        ))
    }

    /// `script` is the direct child; `agy` — which owns the listening socket —
    /// is its child.
    async fn resolve_agy_pid(&mut self) -> Option<u32> {
        if self.agy_pid.is_some() {
            return self.agy_pid;
        }
        let wrapper_pid = self.wrapper.id()?;
        self.agy_pid = child_pid_of(wrapper_pid).await;
        self.agy_pid
    }

    async fn shutdown(&mut self) {
        // Killing the PTY wrapper does not always reap the TUI child, so signal
        // `agy` directly first. Both are Burnrate-owned by construction.
        if let Some(pid) = self.agy_pid {
            let _ = Command::new("/bin/kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        let _ = self.wrapper.kill().await;
    }
}

/// Try an `agy` the user already started. Reuse is best-effort and strictly
/// read-only: a process found here is never signalled.
async fn reuse_running_agy(http: &Client) -> Option<u16> {
    let binary = super::resolve_cli(&agy_binary());
    let deadline = Instant::now();
    for pid in pgrep_full(&binary.to_string_lossy()).await {
        if deadline.elapsed() >= REUSE_TIMEOUT {
            break;
        }
        for port in listening_ports(pid).await {
            let probe = timeout(REUSE_TIMEOUT, rpc(http, port, "RetrieveUserQuotaSummary")).await;
            if let Ok(Ok(value)) = probe
                && value.pointer("/response/groups").is_some()
            {
                return Some(port);
            }
        }
    }
    None
}

async fn child_pid_of(parent: u32) -> Option<u32> {
    let output = Command::new("/usr/bin/pgrep")
        .arg("-P")
        .arg(parent.to_string())
        .output()
        .await
        .ok()?;
    parse_pids(&String::from_utf8_lossy(&output.stdout))
        .into_iter()
        .next()
}

async fn pgrep_full(pattern: &str) -> Vec<u32> {
    let Ok(output) = Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(pattern)
        .output()
        .await
    else {
        return Vec::new();
    };
    parse_pids(&String::from_utf8_lossy(&output.stdout))
}

fn parse_pids(stdout: &str) -> Vec<u32> {
    stdout
        .split_whitespace()
        .filter_map(|value| value.parse::<u32>().ok())
        .collect()
}

/// TCP ports the process is listening on. `agy` opens more than one and only
/// one serves quota, so every candidate is probed.
async fn listening_ports(pid: u32) -> Vec<u16> {
    let Ok(output) = Command::new("/usr/sbin/lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p"])
        .arg(pid.to_string())
        .output()
        .await
    else {
        return Vec::new();
    };
    parse_listening_ports(&String::from_utf8_lossy(&output.stdout))
}

fn parse_listening_ports(stdout: &str) -> Vec<u16> {
    let mut ports = Vec::new();
    for line in stdout.lines() {
        if !line.contains("(LISTEN)") {
            continue;
        }
        let Some(address) = line
            .split_whitespace()
            .find(|field| field.contains(':') && field.rsplit(':').next().is_some())
        else {
            continue;
        };
        if let Some(port) = address
            .rsplit(':')
            .next()
            .and_then(|port| port.parse::<u16>().ok())
            && !ports.contains(&port)
        {
            ports.push(port);
        }
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ProviderKind, SecretStorageMode, SnapshotStatus};
    use serde_json::json;

    fn account() -> AccountConfig {
        AccountConfig {
            id: "antigravity-local".to_string(),
            provider: ProviderKind::Antigravity,
            label: "Antigravity".to_string(),
            enabled: true,
            auto_detected: false,
            credential_path: None,
            endpoint_override: None,
            secret_storage: SecretStorageMode::Keyring,
            keyring_account: None,
            plaintext_secret: None,
            email: None,
            config_dir: None,
            aws_profile: None,
            aws_region: None,
            aws_monthly_budget_usd: None,
            aws_categories: Vec::new(),
            copilot_plan: None,
            copilot_custom_limit: None,
            order_index: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Verbatim `RetrieveUserQuotaSummary` payload captured from `agy` 2.3.1.
    fn summary() -> serde_json::Value {
        json!({
          "response": {
            "groups": [
              {
                "displayName": "Gemini Models",
                "description": "Models within this group: Gemini Flash, Gemini Pro",
                "buckets": [
                  {
                    "bucketId": "gemini-weekly",
                    "displayName": "Weekly Limit Remaining",
                    "description": "You have used some of your weekly limit, it will fully refresh in 6 days, 19 hours.",
                    "window": "weekly",
                    "remainingFraction": 0.9979325,
                    "resetTime": "2026-09-09T00:48:45Z"
                  },
                  {
                    "bucketId": "gemini-5h",
                    "displayName": "Five Hour Limit Remaining",
                    "window": "5h",
                    "remainingFraction": 0.9875948,
                    "resetTime": "2026-09-02T06:41:44Z"
                  }
                ]
              },
              {
                "displayName": "Claude and GPT models",
                "description": "Models within this group: Claude Opus, Claude Sonnet, GPT-OSS",
                "buckets": [
                  {
                    "bucketId": "3p-weekly",
                    "displayName": "Weekly Limit Remaining",
                    "window": "weekly",
                    "remainingFraction": 1,
                    "resetTime": "2026-09-09T05:17:14Z"
                  },
                  {
                    "bucketId": "3p-5h",
                    "displayName": "Five Hour Limit Remaining",
                    "window": "5h",
                    "remainingFraction": 1,
                    "resetTime": "2026-09-02T10:17:14Z"
                  }
                ]
              }
            ]
          }
        })
    }

    /// Verbatim (trimmed) `GetUserStatus` payload from the same session.
    fn user_status() -> serde_json::Value {
        json!({
          "userStatus": {
            "name": "Example User",
            "email": "user@example.com",
            "planStatus": { "planInfo": { "teamsTier": "TEAMS_TIER_PRO", "planName": "Pro" } },
            "userTier": { "name": "Google AI Pro" }
          }
        })
    }

    #[test]
    fn maps_quota_groups_to_percentage_buckets_in_dashboard_order() {
        let snapshot = parse_snapshot(&account(), &summary(), None).unwrap();

        let ids: Vec<&str> = snapshot
            .usage_buckets
            .iter()
            .map(|bucket| bucket.id.as_str())
            .collect();
        assert_eq!(ids, ["gemini-5h", "gemini-weekly", "3p-5h", "3p-weekly"]);

        let gemini_5h = &snapshot.usage_buckets[0];
        // The Gemini pair takes the bare window labels so it wins the dashboard
        // 5-hour / Weekly columns; the Claude + GPT pair stays an extra.
        assert_eq!(gemini_5h.label, "5-hour");
        assert_eq!(gemini_5h.window.as_deref(), Some("5-hour"));
        assert_eq!(gemini_5h.limit, Some(100.0));
        assert_eq!(gemini_5h.unit, "%");
        assert!((gemini_5h.remaining.unwrap() - 98.75948).abs() < 1e-5);
        assert!((gemini_5h.used - 1.24052).abs() < 1e-5);
        assert_eq!(
            gemini_5h.reset_at,
            Some(
                DateTime::parse_from_rfc3339("2026-09-02T06:41:44Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );

        assert_eq!(snapshot.usage_buckets[1].label, "Weekly");
        assert_eq!(snapshot.usage_buckets[2].label, "Claude + GPT 5-hour");
        assert_eq!(snapshot.usage_buckets[3].label, "Claude + GPT Weekly");

        // A nearly untouched account is healthy, and the primary quota mirrors
        // the most immediate Gemini window.
        assert_eq!(snapshot.status, SnapshotStatus::Healthy);
        assert_eq!(snapshot.quota.as_ref().unwrap().limit, Some(100.0));
        assert_eq!(snapshot.email, None);
        assert!(snapshot.subscription.is_none());
    }

    #[test]
    fn adds_identity_and_plan_from_user_status() {
        let snapshot = parse_snapshot(&account(), &summary(), Some(&user_status())).unwrap();

        assert_eq!(snapshot.email.as_deref(), Some("user@example.com"));
        let subscription = snapshot.subscription.unwrap();
        assert_eq!(subscription.plan, SubscriptionPlan::Pro);
        assert_eq!(subscription.plan_label, "Google AI Pro");
        assert_eq!(
            subscription.rate_limit_tier.as_deref(),
            Some("TEAMS_TIER_PRO")
        );
        assert_eq!(subscription.source, "antigravity-cli");
    }

    #[test]
    fn exhausted_and_warning_buckets_follow_the_shared_thresholds() {
        let value = json!({
          "response": { "groups": [{
            "displayName": "Gemini Models",
            "buckets": [
              { "bucketId": "gemini-5h", "window": "5h", "remainingFraction": 0.03 },
              { "bucketId": "gemini-weekly", "window": "weekly", "remainingFraction": 0.15 }
            ]
          }]}
        });

        let snapshot = parse_snapshot(&account(), &value, None).unwrap();

        assert_eq!(snapshot.usage_buckets[0].status, SnapshotStatus::Exhausted);
        assert_eq!(snapshot.usage_buckets[1].status, SnapshotStatus::Warning);
        assert_eq!(snapshot.status, SnapshotStatus::Exhausted);
    }

    #[test]
    fn rejects_payloads_without_usable_buckets() {
        assert!(parse_snapshot(&account(), &json!({}), None).is_err());

        // Groups present, but every bucket lacks a remaining fraction.
        let unusable = json!({
          "response": { "groups": [{
            "displayName": "Gemini Models",
            "buckets": [{ "bucketId": "gemini-5h", "window": "5h", "resetTime": "2026-09-02T06:41:44Z" }]
          }]}
        });
        assert!(parse_snapshot(&account(), &unusable, None).is_err());
    }

    #[test]
    fn skips_buckets_whose_window_is_unrecognized() {
        let value = json!({
          "response": { "groups": [{
            "displayName": "Gemini Models",
            "buckets": [
              { "bucketId": "gemini-daily", "window": "daily", "remainingFraction": 0.5 },
              { "bucketId": "gemini-5h", "window": "5h", "remainingFraction": 0.5 }
            ]
          }]}
        });

        let snapshot = parse_snapshot(&account(), &value, None).unwrap();

        assert_eq!(snapshot.usage_buckets.len(), 1);
        assert_eq!(snapshot.usage_buckets[0].id, "gemini-5h");
    }

    #[test]
    fn reset_time_accepts_iso8601_and_epoch_seconds() {
        assert_eq!(
            parse_reset_time("2026-09-02T06:41:44Z"),
            Some(
                DateTime::parse_from_rfc3339("2026-09-02T06:41:44Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
        assert_eq!(
            parse_reset_time("1772000000"),
            DateTime::from_timestamp(1_772_000_000, 0)
        );
        assert_eq!(parse_reset_time("not a time"), None);
    }

    #[test]
    fn plan_labels_map_to_subscription_tiers() {
        assert_eq!(plan_from_label("Google AI Ultra"), SubscriptionPlan::Max);
        assert_eq!(plan_from_label("Google AI Pro"), SubscriptionPlan::Pro);
        assert_eq!(plan_from_label("Free"), SubscriptionPlan::Free);
        assert_eq!(plan_from_label("Something else"), SubscriptionPlan::Unknown);
    }

    #[test]
    fn parses_listening_ports_from_lsof_output() {
        let stdout = "\
COMMAND   PID     USER   FD   TYPE             DEVICE SIZE/OFF NODE NAME
agy     37607 george.c   10u  IPv4 0x400fc7eb5be83f15      0t0  TCP 127.0.0.1:64366 (LISTEN)
agy     37607 george.c   11u  IPv4 0x68baa41211ab1bf6      0t0  TCP 127.0.0.1:64367 (LISTEN)
agy     37607 george.c   12u  IPv4 0x1111111111111111      0t0  TCP 127.0.0.1:5000->127.0.0.1:6000 (ESTABLISHED)
";

        assert_eq!(parse_listening_ports(stdout), vec![64366, 64367]);
    }

    #[test]
    fn parses_pids_and_ignores_noise() {
        assert_eq!(parse_pids("37605\n37607\n"), vec![37605, 37607]);
        assert_eq!(parse_pids(""), Vec::<u32>::new());
    }

    #[test]
    fn agy_binary_prefers_explicit_overrides() {
        // Deliberately not asserting the default here: the env is process-wide
        // and other tests must not observe a mutated PATH override.
        assert_eq!(agy_binary(), "agy");
    }
}
