import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { RemoteDashboard } from "./RemoteDashboard";
import type { AccountView, UsageSnapshot } from "./types";

afterEach(cleanup);

const summary = {
  label: "1 critical",
  shortLabel: "1 crit",
  status: "exhausted" as const,
  criticalCount: 1,
  warningCount: 0,
  updatedAt: new Date().toISOString(),
};

function account(overrides: Partial<AccountView> = {}): AccountView {
  return {
    id: "claude-code-Claude Max",
    provider: "claude-code",
    label: "Claude Max",
    enabled: true,
    autoDetected: false,
    credentialPath: null,
    endpointOverride: null,
    secretStorage: "keyring",
    hasSecret: false,
    email: null,
    configDir: null,
    subscriptionCostUsd: 100,
    subscriptionRenewsOn: "2026-01-06",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

function snapshot(): UsageSnapshot {
  return {
    accountId: "claude-code-Claude Max",
    provider: "claude-code",
    label: "Claude Max",
    status: "exhausted",
    email: null,
    subscription: null,
    usageBuckets: [],
    quota: null,
    message: null,
    fetchedAt: new Date().toISOString(),
  };
}

test("serves the read-only dashboard, not the account editor", () => {
  render(
    <RemoteDashboard
      accounts={[account()]}
      snapshots={[snapshot()]}
      summary={summary}
      busy={false}
      onRefresh={() => {}}
    />,
  );

  expect(
    screen.getByRole("heading", { name: "Usage Dashboard" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Accounts at a glance" }),
  ).toBeInTheDocument();
  const row = screen.getByRole("row", { name: /Claude Max/ });
  expect(within(row).getByText("$100/mo")).toBeInTheDocument();

  // Everything that writes stays on the desktop.
  expect(screen.queryByTitle("Add account")).not.toBeInTheDocument();
  expect(screen.queryByTitle("Remove account")).not.toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "Web access" })).toBeNull();
});

test("keeps the refresh button honest while a fetch is in flight", () => {
  const onRefresh = vi.fn();
  render(
    <RemoteDashboard
      accounts={[]}
      snapshots={[]}
      summary={summary}
      busy
      onRefresh={onRefresh}
    />,
  );

  expect(screen.getByTitle("Refresh")).toBeDisabled();
  expect(screen.getByText("Loading…")).toBeInTheDocument();
});
