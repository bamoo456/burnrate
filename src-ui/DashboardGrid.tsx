import { AlertCircle, CheckCircle2 } from "lucide-react";
import {
  balanceBuckets,
  balanceUnavailableReason,
  bucketMeterLabel,
  bucketPercent,
  dashboardWindowBucket,
  formatAgo,
  formatBalance,
  formatDashboardMetric,
  formatReset,
  hasAwsCostData,
  nextResetBucket,
  secondaryUsageBuckets,
  type DashboardWindow,
} from "./format";
import { ProviderLogo } from "./ProviderLogo";
import { providerLabels } from "./constants";
import type {
  SnapshotStatus,
  UsageBucketSnapshot,
  UsageSnapshot,
} from "./types";

const statusLabels: Record<SnapshotStatus, string> = {
  healthy: "Healthy",
  warning: "Warning",
  exhausted: "Critical",
  error: "Error",
  stale: "Stale",
  "not-configured": "No accounts",
};

const windows: Array<{ key: DashboardWindow; label: string }> = [
  { key: "5-hour", label: "5-hour" },
  { key: "weekly", label: "Weekly" },
  { key: "monthly", label: "Monthly" },
];

export function DashboardGrid({ snapshots }: { snapshots: UsageSnapshot[] }) {
  return (
    <div className="dashboard-table-wrap">
      <table className="dashboard-table" aria-label="Account usage dashboard">
        <thead>
          <tr>
            <th scope="col">Account</th>
            <th scope="col">Balance</th>
            {windows.map((window) => (
              <th scope="col" key={window.key}>
                {window.label}
              </th>
            ))}
            <th scope="col">Next reset</th>
            <th scope="col">Status</th>
          </tr>
        </thead>
        <tbody>
          {snapshots.map((snapshot) => (
            <DashboardRow key={snapshot.accountId} snapshot={snapshot} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function DashboardRow({ snapshot }: { snapshot: UsageSnapshot }) {
  const resetBucket = nextResetBucket(snapshot);
  return (
    <tr className={snapshot.status}>
      <td className="dashboard-account-cell">
        <AccountIdentity snapshot={snapshot} />
      </td>
      <td>
        <BalanceMetric snapshot={snapshot} />
      </td>
      {windows.map((window) => (
        <td key={window.key}>
          <UsageMetric bucket={dashboardWindowBucket(snapshot, window.key)} />
        </td>
      ))}
      <td>
        <ResetMetric bucket={resetBucket} />
      </td>
      <td className="dashboard-status-cell">
        <StatusBadge status={snapshot.status} />
      </td>
    </tr>
  );
}

function AccountIdentity({ snapshot }: { snapshot: UsageSnapshot }) {
  const plan = snapshot.subscription?.planLabel;
  const extras = secondaryUsageBuckets(snapshot).slice(0, 3);
  return (
    <div className="dashboard-account">
      <ProviderLogo provider={snapshot.provider} size="sm" />
      <div>
        <strong>{snapshot.label}</strong>
        <small>
          {providerLabels[snapshot.provider]}
          {plan ? ` · ${plan}` : ""}
        </small>
        {snapshot.email ? (
          <small className="account-email">{snapshot.email}</small>
        ) : null}
        <small className="snapshot-freshness">
          {hasAwsCostData(snapshot) ? "AWS cost data" : "Updated"} ·{" "}
          {formatAgo(snapshot.fetchedAt)}
        </small>
        {extras.length > 0 ? (
          <div className="dashboard-extras" aria-label="Other usage">
            {extras.map((bucket) => (
              <span key={bucket.id}>
                {bucket.label} · {formatDashboardMetric(bucket)}
              </span>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function BalanceMetric({ snapshot }: { snapshot: UsageSnapshot }) {
  const bucket = balanceBuckets(snapshot)[0] ?? null;
  const unavailable = balanceUnavailableReason(snapshot);
  if (bucket) {
    return (
      <div className={`dashboard-metric balance ${bucket.status}`}>
        <strong>{formatBalance(bucket)}</strong>
        <small>available</small>
      </div>
    );
  }
  if (unavailable) {
    return (
      <div className="dashboard-metric unavailable" title={unavailable}>
        <strong>Unavailable</strong>
        <small>{unavailable}</small>
      </div>
    );
  }
  return <EmptyMetric />;
}

function UsageMetric({ bucket }: { bucket: UsageBucketSnapshot | null }) {
  if (!bucket) return <EmptyMetric />;
  return (
    <div className={`dashboard-metric ${bucket.status}`}>
      <strong>{formatDashboardMetric(bucket)}</strong>
      <div className="dashboard-meter" aria-label={bucketMeterLabel(bucket)}>
        <span style={{ width: `${bucketPercent(bucket)}%` }} />
      </div>
      <small>{formatReset(bucket.resetAt) || bucket.label}</small>
    </div>
  );
}

function ResetMetric({ bucket }: { bucket: UsageBucketSnapshot | null }) {
  if (!bucket?.resetAt) return <EmptyMetric />;
  return (
    <div className="dashboard-metric reset">
      <strong>{formatReset(bucket.resetAt).replace(/^resets /, "")}</strong>
      <small>{bucket.label}</small>
    </div>
  );
}

function EmptyMetric() {
  return (
    <div className="dashboard-metric empty" aria-label="Not available">
      <strong>—</strong>
    </div>
  );
}

function StatusBadge({ status }: { status: SnapshotStatus }) {
  const Icon = status === "healthy" ? CheckCircle2 : AlertCircle;
  return (
    <span className={`status ${status}`}>
      <Icon size={14} />
      {statusLabels[status]}
    </span>
  );
}
