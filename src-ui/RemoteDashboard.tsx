import { RefreshCw } from "lucide-react";
import { DashboardGrid } from "./DashboardGrid";
import { formatAgo } from "./format";
import { DashboardOverview, SectionTitle, type Summary } from "./Preferences";
import type { AccountView, UsageSnapshot } from "./types";

/** The read-only half of the Preferences dashboard, served over the LAN: the
 *  same Overview and "Accounts at a glance" table without the account sidebar,
 *  settings or modals, none of which a remote viewer may touch. Deliberately
 *  not `prefs-shell`, which pins itself to `100vh` because the desktop window
 *  resizes to fit — on a phone that would trap the table in an inner scroller. */
export function RemoteDashboard({
  accounts,
  snapshots,
  summary,
  busy,
  onRefresh,
}: {
  accounts: AccountView[];
  snapshots: UsageSnapshot[];
  summary: Summary;
  busy: boolean;
  onRefresh: () => void;
}) {
  return (
    <main className="remote-shell">
      <header className="prefs-header">
        <div>
          <h1>Usage Dashboard</h1>
          <p>
            <span className="summary-label">{summary.label}</span> · refreshed{" "}
            {formatAgo(summary.updatedAt)}
          </p>
        </div>
        <div className="toolbar">
          <button
            className="icon-button"
            onClick={onRefresh}
            disabled={busy}
            title="Refresh"
          >
            <RefreshCw size={17} className={busy ? "spin" : ""} />
          </button>
        </div>
      </header>

      {accounts.length > 0 || snapshots.length > 0 ? (
        <DashboardOverview
          accounts={accounts}
          snapshots={snapshots}
          summary={summary}
        />
      ) : null}

      <section className="prefs-usage dashboard-section">
        <SectionTitle
          title="Accounts at a glance"
          detail={
            snapshots.length > 0
              ? `${snapshots.length} live account${snapshots.length === 1 ? "" : "s"}`
              : "Idle"
          }
        />
        {snapshots.length > 0 ? (
          <DashboardGrid snapshots={snapshots} accounts={accounts} />
        ) : (
          <div className="empty-state">
            {busy ? "Loading…" : "No accounts are being monitored."}
          </div>
        )}
      </section>
    </main>
  );
}
