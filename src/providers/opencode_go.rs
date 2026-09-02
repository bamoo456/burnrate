use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use reqwest::Client;

use crate::models::{AccountConfig, SnapshotStatus, UsageBucketSnapshot, UsageSnapshot};

use super::{endpoint, overall_status, primary_quota, require_token, status_from_remaining};

const DEFAULT_ENDPOINT: &str = "https://opencode.ai/zen/go/v1/usage";

pub(crate) async fn fetch(http: &Client, account: &AccountConfig) -> Result<UsageSnapshot> {
    let token = require_token(account)?;
    let url = endpoint(account, "BURNRATE_OPENCODE_GO_USAGE_URL", DEFAULT_ENDPOINT)?;
    let value: serde_json::Value = http
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .context("failed to fetch OpenCode Go usage")?
        .error_for_status()
        .context("OpenCode Go usage request failed")?
        .json()
        .await
        .context("failed to decode OpenCode Go usage")?;

    parse_opencode_go(account, &value)
}

pub(crate) fn parse_opencode_go(
    account: &AccountConfig,
    value: &serde_json::Value,
) -> Result<UsageSnapshot> {
    let usage = value
        .get("usage")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("OpenCode Go usage response did not include usage"))?;

    let buckets = vec![
        parse_bucket(usage, "rolling", "5-hour", "5-hour")?,
        parse_bucket(usage, "weekly", "Weekly", "weekly")?,
        parse_bucket(usage, "monthly", "Monthly", "monthly")?,
    ];
    let status = overall_status(&buckets);
    let quota = primary_quota(&buckets);

    Ok(UsageSnapshot {
        account_id: account.id.clone(),
        provider: account.provider,
        label: account.label.clone(),
        status,
        email: None,
        subscription: None,
        usage_buckets: buckets,
        quota,
        message: None,
        fetched_at: Utc::now(),
    })
}

fn parse_bucket(
    usage: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    label: &str,
    window: &str,
) -> Result<UsageBucketSnapshot> {
    let value = usage
        .get(key)
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("OpenCode Go usage response did not include {key} usage"))?;
    let percent = value
        .get("percent")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| anyhow!("OpenCode Go {key} usage did not include a numeric percent"))?
        .clamp(0.0, 100.0);
    let remaining = (100.0 - percent).max(0.0);
    let reset_at = value
        .get("resetsAt")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    let explicitly_rate_limited = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status == "rate-limited");
    let status = if explicitly_rate_limited {
        SnapshotStatus::Exhausted
    } else {
        status_from_remaining(Some(100.0), Some(remaining))
    };

    Ok(UsageBucketSnapshot {
        id: key.to_string(),
        label: label.to_string(),
        window: Some(window.to_string()),
        used: percent,
        limit: Some(100.0),
        remaining: Some(remaining),
        unit: "%".to_string(),
        reset_at,
        status,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;
    use crate::models::{ProviderKind, SecretStorageMode};

    fn account() -> AccountConfig {
        AccountConfig {
            id: "opencode-go-main".to_string(),
            provider: ProviderKind::OpenCodeGo,
            label: "OpenCode Go".to_string(),
            enabled: true,
            auto_detected: false,
            credential_path: None,
            endpoint_override: None,
            secret_storage: SecretStorageMode::Plaintext,
            keyring_account: None,
            plaintext_secret: Some("oc-test".to_string()),
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

    #[test]
    fn maps_official_usage_shape_to_percent_buckets() {
        let snapshot = parse_opencode_go(
            &account(),
            &json!({
                "usage": {
                    "rolling": {
                        "status": "ok",
                        "percent": 42.0,
                        "resetsAt": "2026-09-01T15:30:00Z"
                    },
                    "weekly": {
                        "status": "ok",
                        "percent": 82.0,
                        "resetsAt": "2026-09-05T00:00:00Z"
                    },
                    "monthly": {
                        "status": "rate-limited",
                        "percent": 100.0,
                        "resetsAt": "2026-09-21T00:00:00Z"
                    }
                }
            }),
        )
        .unwrap();

        assert_eq!(snapshot.usage_buckets.len(), 3);
        assert_eq!(snapshot.usage_buckets[0].label, "5-hour");
        assert_eq!(snapshot.usage_buckets[0].used, 42.0);
        assert_eq!(snapshot.usage_buckets[0].remaining, Some(58.0));
        assert_eq!(snapshot.usage_buckets[0].unit, "%");
        assert_eq!(snapshot.usage_buckets[1].status, SnapshotStatus::Warning);
        assert_eq!(snapshot.usage_buckets[2].status, SnapshotStatus::Exhausted);
        assert_eq!(snapshot.status, SnapshotStatus::Exhausted);
        assert_eq!(snapshot.quota.unwrap().remaining, Some(58.0));
    }

    #[test]
    fn rejects_malformed_usage_payloads() {
        let error = parse_opencode_go(
            &account(),
            &json!({
                "usage": {
                    "rolling": { "status": "ok", "percent": 10.0 },
                    "weekly": { "status": "ok" },
                    "monthly": { "status": "ok", "percent": 20.0 }
                }
            }),
        )
        .unwrap_err();

        assert!(error.to_string().contains("weekly"));
        assert!(error.to_string().contains("percent"));
    }

    #[tokio::test]
    async fn fetches_usage_with_bearer_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .and(header("authorization", "Bearer oc-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "usage": {
                    "rolling": { "status": "ok", "percent": 20.0, "resetsAt": "2026-09-01T15:30:00Z" },
                    "weekly": { "status": "ok", "percent": 30.0, "resetsAt": "2026-09-05T00:00:00Z" },
                    "monthly": { "status": "ok", "percent": 40.0, "resetsAt": "2026-09-21T00:00:00Z" }
                }
            })))
            .mount(&server)
            .await;

        let mut account = account();
        account.endpoint_override = Some(server.uri());
        let snapshot = fetch(&Client::new(), &account).await.unwrap();

        assert_eq!(snapshot.usage_buckets.len(), 3);
        assert_eq!(snapshot.usage_buckets[0].remaining, Some(80.0));
        assert_eq!(snapshot.provider, ProviderKind::OpenCodeGo);
    }
}
