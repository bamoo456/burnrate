use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};
use reqwest::Client;
use serde_json::json;

use crate::models::{AccountConfig, UsageBucketSnapshot, UsageSnapshot};

use super::{endpoint, overall_status, primary_quota, status_from_remaining};

const DEFAULT_ENDPOINT: &str =
    "https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage";

/// Cursor publishes no per-user quota API — `api.cursor.com` is team-admin
/// only — so this reads the same Connect-RPC endpoint the IDE dashboard uses,
/// with the access token the signed-in `cursor-agent` CLI stores in the macOS
/// Keychain. Burnrate only ever reads it: the paired `cursor-refresh-token`
/// belongs to the CLI, and a second writer would sign the user out.
pub(crate) async fn fetch(http: &Client, account: &AccountConfig) -> Result<UsageSnapshot> {
    let token = access_token()?;
    let url = endpoint(account, "BURNRATE_CURSOR_USAGE_URL", DEFAULT_ENDPOINT)?;
    let value: serde_json::Value = http
        .post(url)
        .bearer_auth(token)
        .header("Connect-Protocol-Version", "1")
        .json(&json!({}))
        .send()
        .await
        .context("failed to fetch Cursor usage")?
        .error_for_status()
        .context("Cursor usage request failed")?
        .json()
        .await
        .context("failed to decode Cursor usage")?;

    parse_cursor(account, &value)
}

pub(crate) fn parse_cursor(
    account: &AccountConfig,
    value: &serde_json::Value,
) -> Result<UsageSnapshot> {
    let plan = value
        .get("planUsage")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("Cursor usage response did not include planUsage"))?;
    let cents = |key: &str| plan.get(key).and_then(serde_json::Value::as_f64);

    let limit = cents("limit");
    let used = cents("used").or_else(|| Some(limit? - cents("remaining")?));
    let percent = cents("totalPercentUsed");

    // Included usage is a USD pool on current plans, but a seat whose pool the
    // server does not disclose still reports a percentage — fall back to it so
    // the meters stay populated instead of erroring out.
    let bucket = match (limit, used) {
        (Some(limit), Some(used)) if limit > 0.0 => usd_bucket(used / 100.0, limit / 100.0),
        _ => percent_bucket(
            percent.ok_or_else(|| anyhow!("Cursor usage response did not include a plan limit"))?,
        ),
    };

    let reset_at = reset_at(value);
    let bucket = UsageBucketSnapshot { reset_at, ..bucket };
    let buckets = vec![bucket];

    Ok(UsageSnapshot {
        account_id: account.id.clone(),
        provider: account.provider,
        label: account.label.clone(),
        status: overall_status(&buckets),
        email: account.email.clone().or_else(cli_email),
        subscription: None,
        usage_buckets: buckets.clone(),
        quota: primary_quota(&buckets),
        message: value
            .get("displayMessage")
            .and_then(serde_json::Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .map(ToString::to_string),
        fetched_at: Utc::now(),
    })
}

fn usd_bucket(used: f64, limit: f64) -> UsageBucketSnapshot {
    let remaining = (limit - used).max(0.0);
    UsageBucketSnapshot {
        id: "plan".to_string(),
        label: "Monthly".to_string(),
        window: Some("monthly".to_string()),
        used,
        limit: Some(limit),
        remaining: Some(remaining),
        unit: "USD".to_string(),
        reset_at: None,
        status: status_from_remaining(Some(limit), Some(remaining)),
    }
}

fn percent_bucket(percent: f64) -> UsageBucketSnapshot {
    let percent = percent.clamp(0.0, 100.0);
    let remaining = 100.0 - percent;
    UsageBucketSnapshot {
        id: "plan".to_string(),
        label: "Monthly".to_string(),
        window: Some("monthly".to_string()),
        used: percent,
        limit: Some(100.0),
        remaining: Some(remaining),
        unit: "%".to_string(),
        reset_at: None,
        status: status_from_remaining(Some(100.0), Some(remaining)),
    }
}

/// `billingCycleEnd` arrives as a string, and Cursor has shipped it both as
/// epoch millis and as RFC 3339 — accept either.
fn reset_at(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    let raw = value.get("billingCycleEnd").and_then(|value| {
        value
            .as_str()
            .map(ToString::to_string)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
    })?;
    if let Ok(millis) = raw.parse::<i64>() {
        return Utc.timestamp_millis_opt(millis).single();
    }
    DateTime::parse_from_rfc3339(&raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

/// The access token the `cursor-agent` CLI stores in the login keychain.
#[cfg(target_os = "macos")]
fn access_token() -> Result<String> {
    if let Some(token) = env_token() {
        return Ok(token);
    }
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "cursor-access-token",
            "-a",
            "cursor-user",
            "-w",
        ])
        .output()
        .context("failed to run macOS security command")?;
    if !output.status.success() {
        return Err(anyhow!(
            "Cursor credentials not found in Keychain service `cursor-access-token`. Run `cursor-agent login` to sign in."
        ));
    }
    let token = String::from_utf8(output.stdout)
        .context("invalid UTF-8 in Cursor credentials")?
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(anyhow!(
            "Cursor Keychain credential was empty. Run `cursor-agent login` to sign in."
        ));
    }
    Ok(token)
}

#[cfg(not(target_os = "macos"))]
fn access_token() -> Result<String> {
    env_token().ok_or_else(|| {
        anyhow!("Cursor usage tracking currently reads the macOS Keychain and is macOS-only")
    })
}

fn env_token() -> Option<String> {
    std::env::var("BURNRATE_CURSOR_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The signed-in email the CLI caches next to its config. Cosmetic: the usage
/// endpoint does not name the account.
fn cli_email() -> Option<String> {
    let path = std::env::var("CURSOR_CONFIG_DIR")
        .map(PathBuf::from)
        .ok()
        .or_else(|| dirs::home_dir().map(|home| home.join(".cursor")))?
        .join("cli-config.json");
    let config: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    config
        .pointer("/authInfo/email")
        .and_then(serde_json::Value::as_str)
        .filter(|email| !email.trim().is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method},
    };

    use super::*;
    use crate::models::{ProviderKind, SecretStorageMode, SnapshotStatus};

    fn account() -> AccountConfig {
        AccountConfig {
            id: "cursor-main".to_string(),
            provider: ProviderKind::Cursor,
            label: "Cursor".to_string(),
            enabled: true,
            auto_detected: false,
            credential_path: None,
            endpoint_override: None,
            secret_storage: SecretStorageMode::Plaintext,
            keyring_account: None,
            plaintext_secret: None,
            email: Some("dev@example.com".to_string()),
            config_dir: None,
            aws_profile: None,
            aws_region: None,
            aws_monthly_budget_usd: None,
            aws_categories: Vec::new(),
            copilot_plan: None,
            copilot_custom_limit: None,
            subscription_cost_usd: None,
            subscription_renews_on: None,
            order_index: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn maps_included_usd_pool_to_a_monthly_bucket() {
        let snapshot = parse_cursor(
            &account(),
            &json!({
                "billingCycleStart": "1757000000000",
                "billingCycleEnd": "1759591800000",
                "planUsage": { "limit": 2000.0, "used": 1700.0, "totalPercentUsed": 85.0 }
            }),
        )
        .unwrap();

        let bucket = &snapshot.usage_buckets[0];
        assert_eq!(bucket.unit, "USD");
        assert_eq!(bucket.used, 17.0);
        assert_eq!(bucket.limit, Some(20.0));
        assert_eq!(bucket.remaining, Some(3.0));
        assert_eq!(bucket.status, SnapshotStatus::Warning);
        assert_eq!(snapshot.status, SnapshotStatus::Warning);
        assert_eq!(
            bucket.reset_at,
            Utc.timestamp_millis_opt(1_759_591_800_000).single()
        );
    }

    #[test]
    fn derives_used_from_remaining_when_absent() {
        let snapshot = parse_cursor(
            &account(),
            &json!({
                "billingCycleEnd": "2026-10-04T15:30:00Z",
                "planUsage": { "limit": 2000.0, "remaining": 1500.0 }
            }),
        )
        .unwrap();

        let bucket = &snapshot.usage_buckets[0];
        assert_eq!(bucket.used, 5.0);
        assert_eq!(bucket.remaining, Some(15.0));
        assert_eq!(bucket.status, SnapshotStatus::Healthy);
        assert_eq!(
            bucket.reset_at,
            Some(
                DateTime::parse_from_rfc3339("2026-10-04T15:30:00Z")
                    .unwrap()
                    .with_timezone(&Utc)
            )
        );
    }

    #[test]
    fn falls_back_to_percent_when_the_seat_pool_is_undisclosed() {
        let snapshot = parse_cursor(
            &account(),
            &json!({ "planUsage": { "totalPercentUsed": 97.0 } }),
        )
        .unwrap();

        let bucket = &snapshot.usage_buckets[0];
        assert_eq!(bucket.unit, "%");
        assert_eq!(bucket.used, 97.0);
        assert_eq!(bucket.remaining, Some(3.0));
        assert_eq!(bucket.status, SnapshotStatus::Exhausted);
        assert_eq!(bucket.reset_at, None);
    }

    #[test]
    fn rejects_a_response_with_no_usable_usage() {
        let error = parse_cursor(&account(), &json!({ "planUsage": {} })).unwrap_err();
        assert!(error.to_string().contains("plan limit"));

        let error = parse_cursor(&account(), &json!({})).unwrap_err();
        assert!(error.to_string().contains("planUsage"));
    }

    #[tokio::test]
    async fn posts_an_empty_connect_rpc_body_with_the_bearer_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("authorization", "Bearer cursor-test"))
            .and(header("connect-protocol-version", "1"))
            .and(body_json(json!({})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "billingCycleEnd": "1759591800000",
                "planUsage": { "limit": 2000.0, "used": 400.0 }
            })))
            .mount(&server)
            .await;

        let mut account = account();
        account.endpoint_override = Some(server.uri());
        // SAFETY: single-threaded test process for this provider's env override.
        unsafe { std::env::set_var("BURNRATE_CURSOR_TOKEN", "cursor-test") };
        let snapshot = fetch(&Client::new(), &account).await.unwrap();
        unsafe { std::env::remove_var("BURNRATE_CURSOR_TOKEN") };

        assert_eq!(snapshot.provider, ProviderKind::Cursor);
        assert_eq!(snapshot.usage_buckets[0].remaining, Some(16.0));
    }
}
