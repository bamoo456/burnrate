//! Burn-rate estimation from locally sampled quota counters.
//!
//! Provider APIs only expose point-in-time values — Codex returns a credits
//! balance or a period-to-date credit allowance, never a spend rate — so the
//! rate has to come from differences between samples persisted across refresh
//! cycles (see `storage::UsageSample`). Runpod is the exception: its API
//! reports `currentSpendPerHr` directly and never goes through this module.

use chrono::{DateTime, Duration, Utc};

use crate::models::{ProviderKind, UsageBucketSnapshot};

/// Longest history consulted before giving up on a rate. The store keeps more,
/// but a week-old slope is not a current burn rate.
const PRIMARY_WINDOW_SECONDS: i64 = 24 * 60 * 60;
const FALLBACK_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
/// Two samples closer together than this describe noise, not a trend.
const MIN_SPAN_SECONDS: i64 = 30 * 60;

/// Which bucket value is the counter to difference over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SampleKind {
    /// Monotonic within a quota period (e.g. monthly credits used). A drop
    /// means the period reset, so only the latest run is differenced.
    Cumulative,
    /// Spendable balance that decreases with use and jumps up on top-ups
    /// (e.g. purchased credits); increases are ignored.
    Balance,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SamplePoint {
    pub value: f64,
    pub observed_at: DateTime<Utc>,
}

/// Sample kinds this provider/bucket participates in. Percentage windows,
/// token counts, and API-provided rates (Runpod) are deliberately excluded.
pub(crate) fn sample_kind(
    provider: ProviderKind,
    bucket: &UsageBucketSnapshot,
) -> Option<SampleKind> {
    if provider != ProviderKind::Codex {
        return None;
    }
    if bucket.id.ends_with("-credits-monthly") {
        return Some(SampleKind::Cumulative);
    }
    if bucket.id.ends_with("-credits") {
        return Some(SampleKind::Balance);
    }
    None
}

/// The value to persist for a sampled bucket.
pub(crate) fn sample_value(provider: ProviderKind, bucket: &UsageBucketSnapshot) -> Option<f64> {
    let value = match sample_kind(provider, bucket)? {
        SampleKind::Cumulative => bucket.used,
        SampleKind::Balance => bucket.remaining?,
    };
    value.is_finite().then_some(value)
}

/// Estimated burn note (`burn ~X credits/hr · runway Yd`) for a bucket from
/// samples sorted oldest-first. `None` until enough history spans the minimum
/// window — a fresh install shows nothing rather than a wild guess.
pub(crate) fn burn_note(
    kind: SampleKind,
    bucket: &UsageBucketSnapshot,
    samples: &[SamplePoint],
) -> Option<String> {
    let remaining = bucket.remaining.filter(|value| *value > 0.0)?;
    let window = select_window(samples, PRIMARY_WINDOW_SECONDS)
        .or_else(|| select_window(samples, FALLBACK_WINDOW_SECONDS))?;
    let (consumed, elapsed_seconds) = match kind {
        SampleKind::Cumulative => cumulative_consumption(window)?,
        SampleKind::Balance => balance_consumption(window),
    };
    let rate_per_hour = consumed / (elapsed_seconds / 3600.0);
    if !rate_per_hour.is_finite() || rate_per_hour <= 0.0 {
        return None;
    }
    let runway_seconds = remaining / rate_per_hour * 3600.0;
    Some(format!(
        "burn ~{}/hr · runway {}",
        format_rate(rate_per_hour, &bucket.unit),
        format_runway(runway_seconds),
    ))
}

/// The trailing slice ending at the newest sample, when it spans enough time.
fn select_window(samples: &[SamplePoint], seconds: i64) -> Option<&[SamplePoint]> {
    let last = samples.last()?;
    let cutoff = last.observed_at - Duration::seconds(seconds);
    let start = samples.partition_point(|sample| sample.observed_at < cutoff);
    let window = &samples[start..];
    if window.len() < 2 {
        return None;
    }
    let span = (window.last()?.observed_at - window.first()?.observed_at).num_seconds();
    (span >= MIN_SPAN_SECONDS).then_some(window)
}

/// Only the latest monotonic run counts: when a cumulative counter drops, the
/// quota period reset and pre-reset consumption must not dilute the rate.
fn cumulative_consumption(window: &[SamplePoint]) -> Option<(f64, f64)> {
    let mut start = 0;
    for index in 1..window.len() {
        if window[index].value < window[index - 1].value {
            start = index;
        }
    }
    let first = window.get(start)?;
    let last = window.last()?;
    let elapsed = (last.observed_at - first.observed_at).num_seconds() as f64;
    let consumed = last.value - first.value;
    (elapsed >= MIN_SPAN_SECONDS as f64 && consumed > 0.0).then_some((consumed, elapsed))
}

/// Decreases count as burn; a top-up increase is ignored, but still defines
/// the elapsed span.
fn balance_consumption(window: &[SamplePoint]) -> (f64, f64) {
    let consumed = window
        .windows(2)
        .map(|pair| (pair[0].value - pair[1].value).max(0.0))
        .sum();
    let elapsed =
        (window[window.len() - 1].observed_at - window[0].observed_at).num_seconds() as f64;
    (consumed, elapsed)
}

fn format_rate(rate_per_hour: f64, unit: &str) -> String {
    let amount = if rate_per_hour >= 10.0 {
        format!("{rate_per_hour:.0}")
    } else {
        format!("{rate_per_hour:.1}")
    };
    format!("{amount} {unit}")
}

/// Compact runway label, shared with the Runpod provider's snapshot message.
pub(crate) fn format_runway(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{:.0}s", seconds.max(0.0));
    }
    if seconds < 60.0 * 60.0 {
        return format!("{:.0}m", seconds / 60.0);
    }
    if seconds < 48.0 * 60.0 * 60.0 {
        return format!("{:.1}h", seconds / 60.0 / 60.0);
    }
    format!("{:.0}d", seconds / 60.0 / 60.0 / 24.0)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::models::SnapshotStatus;

    fn at(minutes: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 12, 0, 0, 0).unwrap() + Duration::minutes(minutes)
    }

    fn point(value: f64, minutes: i64) -> SamplePoint {
        SamplePoint {
            value,
            observed_at: at(minutes),
        }
    }

    fn bucket(id: &str, used: f64, remaining: Option<f64>) -> UsageBucketSnapshot {
        UsageBucketSnapshot {
            id: id.to_string(),
            label: "Monthly credits".to_string(),
            window: Some("Monthly".to_string()),
            used,
            limit: remaining.map(|remaining| used + remaining),
            remaining,
            unit: "credits".to_string(),
            reset_at: None,
            status: SnapshotStatus::Healthy,
        }
    }

    #[test]
    fn cumulative_rate_uses_the_whole_window_and_scales_to_runway() {
        let samples = vec![point(100.0, 0), point(160.0, 60), point(220.0, 120)];
        let note = burn_note(
            SampleKind::Cumulative,
            &bucket("codex-credits-monthly", 220.0, Some(1_200.0)),
            &samples,
        )
        .unwrap();

        assert_eq!(note, "burn ~60 credits/hr · runway 20.0h");
    }

    #[test]
    fn cumulative_rate_ignores_pre_reset_history() {
        // 900 -> 1000 in one period, reset to 0, then 50 consumed after reset.
        let samples = vec![
            point(900.0, 0),
            point(1_000.0, 30),
            point(0.0, 60),
            point(50.0, 120),
        ];
        let note = burn_note(
            SampleKind::Cumulative,
            &bucket("codex-credits-monthly", 50.0, Some(500.0)),
            &samples,
        )
        .unwrap();

        assert_eq!(note, "burn ~50 credits/hr · runway 10.0h");
    }

    #[test]
    fn balance_rate_ignores_top_ups() {
        // Burn 20, top up to 200, then burn another 30.
        let samples = vec![
            point(100.0, 0),
            point(80.0, 30),
            point(200.0, 60),
            point(170.0, 90),
        ];
        let note = burn_note(
            SampleKind::Balance,
            &bucket("codex-credits", 170.0, Some(170.0)),
            &samples,
        )
        .unwrap();

        assert_eq!(note, "burn ~33 credits/hr · runway 5.1h");
    }

    #[test]
    fn falls_back_to_the_week_window_when_recent_samples_are_too_few() {
        // Two samples three days apart: only one lands in the 24h window.
        let samples = vec![point(100.0, 0), point(160.0, 3 * 24 * 60)];
        let note = burn_note(
            SampleKind::Cumulative,
            &bucket("codex-credits-monthly", 160.0, Some(60.0)),
            &samples,
        )
        .unwrap();

        assert_eq!(note, "burn ~0.8 credits/hr · runway 3d");
    }

    #[test]
    fn needs_two_samples_spanning_the_minimum_window() {
        let bucket = bucket("codex-credits-monthly", 10.0, Some(90.0));
        assert!(burn_note(SampleKind::Cumulative, &bucket, &[]).is_none());
        assert!(burn_note(SampleKind::Cumulative, &bucket, &[point(1.0, 0)]).is_none());
        assert!(
            burn_note(
                SampleKind::Cumulative,
                &bucket,
                &[point(1.0, 0), point(2.0, 5)],
            )
            .is_none(),
            "five minutes of samples is noise"
        );
    }

    #[test]
    fn no_note_without_burn_or_remaining_credits() {
        let flat = vec![point(10.0, 0), point(10.0, 60)];
        assert!(
            burn_note(
                SampleKind::Cumulative,
                &bucket("codex-credits-monthly", 10.0, Some(90.0)),
                &flat,
            )
            .is_none()
        );

        let burning = vec![point(10.0, 0), point(40.0, 60)];
        assert!(
            burn_note(
                SampleKind::Cumulative,
                &bucket("codex-credits-monthly", 40.0, Some(0.0)),
                &burning,
            )
            .is_none(),
            "nothing left to project"
        );
    }

    #[test]
    fn sample_selection_is_scoped_to_codex_credit_buckets() {
        let monthly = bucket("codex-credits-monthly", 10.0, Some(90.0));
        assert_eq!(
            sample_value(ProviderKind::Codex, &monthly),
            Some(10.0),
            "cumulative buckets sample the used counter"
        );
        assert_eq!(
            sample_value(
                ProviderKind::Codex,
                &bucket("codex-credits", 5.0, Some(5.0))
            ),
            Some(5.0),
            "balance buckets sample the remaining balance"
        );
        let prefixed = bucket("codex_bengalfox-credits-monthly", 1.0, Some(9.0));
        assert_eq!(
            sample_value(ProviderKind::Codex, &prefixed),
            Some(1.0),
            "extra limit groups are sampled too"
        );
        assert!(
            sample_value(
                ProviderKind::Codex,
                &bucket("codex-primary", 50.0, Some(50.0))
            )
            .is_none()
        );
        assert!(
            sample_value(
                ProviderKind::OpenRouter,
                &bucket("codex-credits-monthly", 1.0, Some(9.0)),
            )
            .is_none()
        );
    }

    #[test]
    fn formats_runway_in_readable_units() {
        assert_eq!(format_runway(30.0), "30s");
        assert_eq!(format_runway(90.0), "2m");
        assert_eq!(format_runway(2.0 * 3600.0), "2.0h");
        assert_eq!(format_runway(72.0 * 3600.0), "3d");
    }
}
