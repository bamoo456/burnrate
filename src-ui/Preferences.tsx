import {
  AlertCircle,
  CheckCircle2,
  DownloadCloud,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { AccountModal, type AccountModalMode } from "./AccountModal";
import { DashboardGrid } from "./DashboardGrid";
import {
  formatAgo,
  formatMonthlyCostUsd,
  totalMonthlySubscriptionCost,
} from "./format";
import { ProviderLogo } from "./ProviderLogo";
import { SortableList } from "./SortableList";
import { UpdateBanner } from "./UpdateBanner";
import { PROVIDERS, providerLabels } from "./constants";
import type {
  AccountInput,
  AccountView,
  ProviderKind,
  SnapshotStatus,
  UpdateChannel,
  UsageSnapshot,
} from "./types";
import type { UpdaterState } from "./useUpdater";

/** Update-channel + auto-updater wiring passed down from `App`. */
export interface UpdatesPanelProps {
  channel: UpdateChannel;
  state: UpdaterState;
  appVersion: string;
  onChannelChange: (channel: UpdateChannel) => void;
  onCheck: () => void;
  onInstall: () => void;
  onDismiss: () => void;
}

export interface TraySettingsPanelProps {
  trayScale: number;
  onTrayScaleChange: (scale: number) => void;
}

export interface RemoteAccessPanelProps {
  enabled: boolean;
  /** Link to open on the phone; `null` until the backend has minted a token. */
  shareUrl: string | null;
  onEnabledChange: (enabled: boolean) => void;
  onOpenShareUrl: () => void;
}

const statusLabels: Record<SnapshotStatus, string> = {
  healthy: "Healthy",
  warning: "Warning",
  exhausted: "Critical",
  error: "Error",
  stale: "Stale",
  "not-configured": "No accounts",
};

type Summary = {
  label: string;
  shortLabel: string;
  status: SnapshotStatus;
  criticalCount: number;
  warningCount: number;
  updatedAt: string;
};

export function Preferences({
  accounts,
  snapshots,
  summary,
  busy,
  error,
  onSaveAccount,
  onToggleAccount,
  onRefresh,
  onRemoveAccount,
  onStartLogin,
  onLogout,
  onReorderAccounts,
  settings,
  remote,
  updates,
}: {
  accounts: AccountView[];
  snapshots: UsageSnapshot[];
  summary: Summary;
  busy: boolean;
  error: string | null;
  /** Resolves on success; rejections render inline in the account modal. */
  onSaveAccount: (input: AccountInput) => Promise<void>;
  onToggleAccount: (account: AccountView) => void;
  onRefresh: () => void;
  onRemoveAccount: (id: string) => void;
  onStartLogin: (provider: ProviderKind, accountId?: string) => void;
  onLogout: (id: string) => void;
  onReorderAccounts: (orderedIds: string[]) => void;
  settings: TraySettingsPanelProps;
  remote: RemoteAccessPanelProps;
  updates: UpdatesPanelProps;
}) {
  const [modal, setModal] = useState<
    null | { kind: "add" } | { kind: "edit"; accountId: string }
  >(null);
  // Resolve the edit target from live account state so the modal reflects
  // backend updates (and vanishes if the account is removed elsewhere).
  const editingAccount =
    modal?.kind === "edit"
      ? accounts.find((account) => account.id === modal.accountId)
      : undefined;
  const modalMode: AccountModalMode | null =
    modal?.kind === "add"
      ? { kind: "add" }
      : editingAccount
        ? { kind: "edit", account: editingAccount }
        : null;
  // New browser sign-ins get an isolated CLI home. Legacy auto-detected
  // system-default accounts are intentionally not eligible for re-auth because
  // that would target ~/.claude / ~/.codex rather than Burnrate-owned state.
  const canReauth = !editingAccount || Boolean(editingAccount.configDir);

  return (
    <main className="prefs-shell">
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

      <UpdateBanner
        state={updates.state}
        channel={updates.channel}
        onInstall={updates.onInstall}
        onDismiss={updates.onDismiss}
        onRetry={updates.onInstall}
      />

      {error ? (
        <div className="notice error" role="alert">
          <AlertCircle size={18} />
          <span>{error}</span>
        </div>
      ) : null}

      <section className="prefs-layout">
        <aside className="prefs-list" aria-label="Accounts">
          <div className="section-heading">
            <h2>Accounts</h2>
            <div className="section-actions">
              <span>{accounts.length}</span>
              <button
                className="icon-button subtle"
                title="Add account"
                onClick={() => setModal({ kind: "add" })}
              >
                <Plus size={16} />
              </button>
            </div>
          </div>

          <SortableList
            items={accounts}
            onReorder={onReorderAccounts}
            ariaLabel="Account order"
            className="account-list"
            renderItem={(account, handle) => (
              <AccountButton
                account={account}
                handle={handle}
                active={
                  modal?.kind === "edit" && modal.accountId === account.id
                }
                onEdit={(target) =>
                  setModal({ kind: "edit", accountId: target.id })
                }
                onToggle={onToggleAccount}
                onRemove={onRemoveAccount}
              />
            )}
          />
          {accounts.length === 0 ? (
            <p className="muted">No accounts configured.</p>
          ) : null}
        </aside>

        <section className="prefs-main" aria-label="Usage and account settings">
          {accounts.length > 0 || snapshots.length > 0 ? (
            <DashboardOverview
              accounts={accounts}
              snapshots={snapshots}
              summary={summary}
            />
          ) : null}

          {accounts.length === 0 && snapshots.length === 0 && !busy ? (
            <FirstRunPanel onAdd={() => setModal({ kind: "add" })} />
          ) : (
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
              ) : !busy ? (
                <div className="empty-state">
                  Add an account to start monitoring quota.
                </div>
              ) : null}
            </section>
          )}

          <SectionTitle title="Preferences" detail="Configuration" />

          <section className="prefs-insights" aria-label="Credential policy">
            <SectionTitle
              title="Credential policy"
              detail="Account-managed only"
            />
            <p className="muted">
              Burnrate does not auto-detect system CLI accounts or scan local
              Claude Code, Codex, or Copilot session history. Credentials must
              be added explicitly and secrets are stored in the OS keyring.
            </p>
          </section>

          <TraySettings {...settings} />

          <RemoteAccessSettings {...remote} />

          <UpdatesSettings {...updates} />
        </section>
      </section>

      {modalMode ? (
        <AccountModal
          key={
            modalMode.kind === "edit" ? `edit-${modalMode.account.id}` : "add"
          }
          mode={modalMode}
          busy={busy}
          canReauth={canReauth}
          onSave={onSaveAccount}
          onClose={() => setModal(null)}
          onStartLogin={onStartLogin}
          onLogout={onLogout}
          onRemove={onRemoveAccount}
        />
      ) : null}
    </main>
  );
}

function DashboardOverview({
  accounts,
  snapshots,
  summary,
}: {
  accounts: AccountView[];
  snapshots: UsageSnapshot[];
  summary: Summary;
}) {
  const enabledCount = accounts.filter((account) => account.enabled).length;
  const healthyCount = snapshots.filter(
    (snapshot) => snapshot.status === "healthy",
  ).length;
  const staleCount = snapshots.filter(
    (snapshot) => snapshot.status === "stale",
  ).length;
  // Disabled accounts are included on purpose: the subscription still bills
  // while the account is unmonitored.
  const totalCost = totalMonthlySubscriptionCost(accounts);

  return (
    <section className="prefs-usage" aria-label="Dashboard overview">
      <SectionTitle
        title="Overview"
        detail={`Refreshed ${formatAgo(summary.updatedAt)}`}
      />
      <div className="usage-list">
        <article className={`usage-row ${summary.status}`}>
          <div className="usage-head">
            <div className="usage-provider">
              <span>
                <strong>
                  {enabledCount} enabled account{enabledCount === 1 ? "" : "s"}
                </strong>
                <small>
                  {healthyCount} healthy · {summary.warningCount} warning ·{" "}
                  {summary.criticalCount} critical
                  {staleCount > 0 ? ` · ${staleCount} stale` : ""}
                  {totalCost > 0
                    ? ` · ${formatMonthlyCostUsd(totalCost)} subscriptions`
                    : ""}
                </small>
              </span>
            </div>
            <StatusBadge status={summary.status} />
          </div>
        </article>
      </div>
    </section>
  );
}

/** Guided empty state for a fresh install: no accounts configured yet. */
function FirstRunPanel({ onAdd }: { onAdd: () => void }) {
  return (
    <section className="prefs-usage first-run" aria-label="Get started">
      <SectionTitle title="Get started" detail="No accounts yet" />
      <div className="first-run-body">
        <div className="first-run-logos" aria-hidden="true">
          {PROVIDERS.map((provider) => (
            <ProviderLogo key={provider} provider={provider} size="sm" />
          ))}
        </div>
        <p className="muted">
          Add each account explicitly. Claude Code and Codex browser sign-ins
          are isolated from your system-default CLI sessions; API keys are kept
          in the operating system keyring.
        </p>
        <div className="first-run-actions">
          <button className="primary" onClick={onAdd}>
            <Plus size={15} /> Add your first account
          </button>
        </div>
      </div>
    </section>
  );
}

function TraySettings({
  trayScale,
  onTrayScaleChange,
}: TraySettingsPanelProps) {
  const percent = Math.round(trayScale * 100);
  return (
    <section className="prefs-tray-settings" aria-label="Tray popover">
      <SectionTitle title="Tray popover" detail="Sizing" />
      <label className="settings-slider">
        <span>
          <strong>Tray content scale</strong>
          <small>
            {percent === 100
              ? "Native size — scaling disabled"
              : `${percent}% — scales down before showing an internal scrollbar`}
          </small>
        </span>
        <input
          type="range"
          min="0.5"
          max="1"
          step="0.05"
          value={trayScale}
          onChange={(event) => onTrayScaleChange(Number(event.target.value))}
        />
      </label>
    </section>
  );
}

function RemoteAccessSettings({
  enabled,
  shareUrl,
  onEnabledChange,
  onOpenShareUrl,
}: RemoteAccessPanelProps) {
  const [copied, setCopied] = useState<"ok" | "failed" | null>(null);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(shareUrl ?? "");
      setCopied("ok");
    } catch {
      setCopied("failed");
    }
    setTimeout(() => setCopied(null), 2000);
  };
  return (
    <section className="prefs-remote" aria-label="Web access">
      <SectionTitle title="Web access" detail="Read-only" />
      <label className="settings-toggle">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => onEnabledChange(event.target.checked)}
        />
        <span>
          <strong>Serve the tray view on this network</strong>
          <small>
            Open the link below on a phone or another computer to watch quota.
            The page is read-only; anyone with the link can see your usage and
            account emails, so keep it to a trusted network or Tailscale.
          </small>
        </span>
      </label>
      {enabled && shareUrl ? (
        <p className="remote-share-url">
          {/* Deliberately not an <a href>: WKWebView turns a drag on a link
              into a link-drag (killing text selection) and its context-menu
              "Open Link" would navigate this window away, which a
              config-declared window cannot refuse. The backend opens it. */}
          <button type="button" className="share-link" onClick={onOpenShareUrl}>
            {shareUrl}
          </button>
          <button type="button" onClick={() => void copy()}>
            {copied === "ok"
              ? "Copied"
              : copied === "failed"
                ? "Copy failed"
                : "Copy link"}
          </button>
        </p>
      ) : null}
    </section>
  );
}

function UpdatesSettings({
  channel,
  state,
  appVersion,
  onChannelChange,
  onCheck,
  onInstall,
}: UpdatesPanelProps) {
  // Check feedback (checking / up to date / failed) lives in the
  // UpdateDialog that `onCheck` opens; the banner owns the passive
  // "update available" affordance.
  return (
    <section className="prefs-updates" aria-label="Updates">
      <SectionTitle title="Updates" detail={`v${appVersion}`} />
      <div className="updates-body">
        <label className="updates-channel">
          <span>Release channel</span>
          <select
            value={channel}
            onChange={(event) =>
              onChannelChange(event.target.value as UpdateChannel)
            }
          >
            <option value="stable">Stable</option>
            <option value="nightly">Nightly (latest, less stable)</option>
          </select>
        </label>
        <p className="muted updates-note">
          Stable tracks signed releases. Nightly follows the rolling{" "}
          <code>nightly</code> build — newer features, fewer guarantees.
        </p>
        <div className="updates-actions">
          <button
            className="secondary"
            onClick={onCheck}
            disabled={state.checking || state.downloading}
          >
            <RefreshCw size={14} className={state.checking ? "spin" : ""} />
            Check for updates
          </button>
          {state.available ? (
            <button
              className="primary"
              onClick={onInstall}
              disabled={state.downloading}
            >
              <DownloadCloud size={14} />
              {state.downloading
                ? `Installing… ${state.progress}%`
                : "Install & Restart"}
            </button>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function SectionTitle({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="section-heading">
      <h2>{title}</h2>
      <span>{detail}</span>
    </div>
  );
}

function AccountButton({
  account,
  handle,
  active,
  onEdit,
  onToggle,
  onRemove,
}: {
  account: AccountView;
  handle: ReactNode;
  active: boolean;
  onEdit: (account: AccountView) => void;
  onToggle: (account: AccountView) => void;
  onRemove: (id: string) => void;
}) {
  const [confirmRemove, setConfirmRemove] = useState(false);

  return (
    <div className={`account-row ${active ? "active" : ""}`}>
      {handle}
      <button
        className="account-main"
        title={
          account.email ? `${account.label} — ${account.email}` : account.label
        }
        onClick={() => onEdit(account)}
      >
        <ProviderLogo provider={account.provider} size="sm" />
        <span>
          <strong>{account.label}</strong>
          <small>
            {providerLabels[account.provider]}
            {account.autoDetected ? " · Legacy auto-detected" : ""}
          </small>
          {account.email ? (
            <small className="account-email">{account.email}</small>
          ) : null}
        </span>
      </button>
      {confirmRemove ? (
        <span className="account-flags remove-confirm">
          <button
            className="danger-solid"
            title={`Confirm removing ${account.label}`}
            onClick={() => onRemove(account.id)}
          >
            Remove
          </button>
          <button
            className="secondary"
            title="Keep account"
            onClick={() => setConfirmRemove(false)}
          >
            Keep
          </button>
        </span>
      ) : (
        <span className="account-flags">
          <button
            type="button"
            role="switch"
            aria-checked={account.enabled}
            aria-label={account.enabled ? "Enabled" : "Disabled"}
            className={`row-switch${account.enabled ? " on" : ""}`}
            onClick={() => onToggle(account)}
          >
            <span />
          </button>
          <button
            className="icon-button danger"
            title="Remove account"
            onClick={() => setConfirmRemove(true)}
          >
            <Trash2 size={15} />
          </button>
        </span>
      )}
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
