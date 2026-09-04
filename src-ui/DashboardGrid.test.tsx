import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { DashboardGrid } from "./DashboardGrid";
import type { AccountView, UsageSnapshot } from "./types";

afterEach(() => cleanup());

function snapshot(
  overrides: Partial<UsageSnapshot> & Pick<UsageSnapshot, "provider" | "label">,
): UsageSnapshot {
  return {
    accountId: `${overrides.provider}-${overrides.label}`,
    status: "healthy",
    email: null,
    subscription: null,
    usageBuckets: [],
    quota: null,
    message: null,
    fetchedAt: new Date().toISOString(),
    ...overrides,
  };
}

function account(
  overrides: Partial<AccountView> & Pick<AccountView, "id" | "label">,
): AccountView {
  return {
    provider: "claude-code",
    enabled: true,
    autoDetected: false,
    credentialPath: null,
    endpointOverride: null,
    secretStorage: "keyring",
    hasSecret: false,
    email: null,
    configDir: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

test("renders fixed dashboard columns and normalizes OpenCode Go windows", () => {
  render(
    <DashboardGrid
      snapshots={[
        snapshot({
          provider: "opencode-go",
          label: "OpenCode Go",
          usageBuckets: [
            {
              id: "rolling",
              label: "5-hour",
              window: "5-hour",
              used: 42,
              limit: 100,
              remaining: 58,
              unit: "%",
              resetAt: "2026-09-02T01:00:00Z",
              status: "healthy",
            },
            {
              id: "weekly",
              label: "Weekly",
              window: "weekly",
              used: 70,
              limit: 100,
              remaining: 30,
              unit: "%",
              resetAt: "2026-09-05T01:00:00Z",
              status: "healthy",
            },
            {
              id: "monthly",
              label: "Monthly",
              window: "monthly",
              used: 15,
              limit: 100,
              remaining: 85,
              unit: "%",
              resetAt: "2026-10-01T00:00:00Z",
              status: "healthy",
            },
          ],
        }),
      ]}
    />,
  );

  expect(screen.getByRole("columnheader", { name: "Balance" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "5-hour" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "Weekly" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "Monthly" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "Next reset" })).toBeInTheDocument();

  const row = screen.getByRole("row", { name: /OpenCode Go/ });
  expect(within(row).getByText("Unavailable")).toBeInTheDocument();
  expect(within(row).getByText("58% left")).toBeInTheDocument();
  expect(within(row).getByText("30% left")).toBeInTheDocument();
  expect(within(row).getByText("85% left")).toBeInTheDocument();
});

test("shows OpenRouter balance without inventing quota windows", () => {
  render(
    <DashboardGrid
      snapshots={[
        snapshot({
          provider: "openrouter",
          label: "OpenRouter Team",
          usageBuckets: [
            {
              id: "credits",
              label: "Credits",
              window: null,
              used: 7.25,
              limit: 25,
              remaining: 17.75,
              unit: "USD",
              resetAt: null,
              status: "healthy",
            },
          ],
          quota: {
            used: 7.25,
            limit: 25,
            remaining: 17.75,
            unit: "USD",
            resetAt: null,
          },
        }),
      ]}
    />,
  );

  const row = screen.getByRole("row", { name: /OpenRouter Team/ });
  expect(within(row).getByText("$17.75")).toBeInTheDocument();
  expect(within(row).getAllByLabelText("Not available")).toHaveLength(5);
});

test("maps AWS month-to-date into Monthly and keeps category details visible", () => {
  render(
    <DashboardGrid
      snapshots={[
        snapshot({
          provider: "aws",
          label: "AWS",
          status: "warning",
          usageBuckets: [
            {
              id: "aws-mtd",
              label: "AWS month-to-date",
              window: "month-to-date",
              used: 164.25,
              limit: 200,
              remaining: 35.75,
              unit: "USD",
              resetAt: null,
              status: "warning",
            },
            {
              id: "aws-category-bedrock",
              label: "Bedrock",
              window: "month-to-date",
              used: 48.5,
              limit: null,
              remaining: null,
              unit: "USD",
              resetAt: null,
              status: "healthy",
            },
            {
              id: "aws-category-ec2-compute",
              label: "EC2 compute",
              window: "month-to-date",
              used: 72.1,
              limit: null,
              remaining: null,
              unit: "USD",
              resetAt: null,
              status: "healthy",
            },
          ],
        }),
      ]}
    />,
  );

  const row = screen.getByRole("row", { name: /AWS/ });
  expect(within(row).getByText("$35.75 left")).toBeInTheDocument();
  expect(within(row).getByText("Bedrock")).toBeInTheDocument();
  expect(within(row).getByText("$48.50 used")).toBeInTheDocument();
  expect(within(row).getByText("EC2 compute")).toBeInTheDocument();
  expect(within(row).getByText("$72.10 used")).toBeInTheDocument();
  expect(within(row).getByText("Warning")).toBeInTheDocument();
});

test("shows per-account subscription cost and a total that includes disabled accounts", () => {
  render(
    <DashboardGrid
      snapshots={[
        snapshot({ provider: "claude-code", label: "Claude Max" }),
        snapshot({ provider: "codex", label: "Codex Pro" }),
      ]}
      accounts={[
        account({ id: "claude-code-Claude Max", label: "Claude Max", subscriptionCostUsd: 100 }),
        account({
          id: "codex-Codex Pro",
          label: "Codex Pro",
          enabled: false,
          subscriptionCostUsd: 19.99,
        }),
      ]}
    />,
  );

  const claudeRow = screen.getByRole("row", { name: /Claude Max/ });
  expect(within(claudeRow).getByText("$100/mo")).toBeInTheDocument();
  expect(within(claudeRow).getByText("subscription")).toBeInTheDocument();
  const codexRow = screen.getByRole("row", { name: /Codex Pro/ });
  expect(within(codexRow).getByText("$19.99/mo")).toBeInTheDocument();

  expect(screen.getByRole("columnheader", { name: "Cost" })).toBeInTheDocument();
  const total = screen.getByRole("row", { name: /Total subscriptions/ });
  // 100 + 19.99 — the disabled Codex account still bills, so it counts.
  expect(within(total).getByText("$119.99/mo")).toBeInTheDocument();
});

test("omits the cost column values and total when no account has a cost", () => {
  render(
    <DashboardGrid
      snapshots={[snapshot({ provider: "openrouter", label: "OpenRouter" })]}
      accounts={[account({ id: "openrouter-OpenRouter", label: "OpenRouter" })]}
    />,
  );

  expect(screen.getByRole("columnheader", { name: "Cost" })).toBeInTheDocument();
  expect(screen.getByRole("row", { name: /OpenRouter/ }).textContent).not.toContain("/mo");
  expect(screen.queryByRole("row", { name: /Total subscriptions/ })).not.toBeInTheDocument();
});

test("rolls a stored renewal anchor forward to the next billing date", () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 8, 4));
  try {
    render(
      <DashboardGrid
        snapshots={[
          snapshot({ provider: "claude-code", label: "Claude Max" }),
          snapshot({ provider: "codex", label: "Codex Pro" }),
        ]}
        accounts={[
          account({
            id: "claude-code-Claude Max",
            label: "Claude Max",
            // Months in the past: the anchor still resolves to this month.
            subscriptionRenewsOn: "2025-03-23",
          }),
          account({ id: "codex-Codex Pro", label: "Codex Pro" }),
        ]}
      />,
    );

    expect(
      screen.getByRole("columnheader", { name: "Renews" }),
    ).toBeInTheDocument();
    const claudeRow = screen.getByRole("row", { name: /Claude Max/ });
    expect(within(claudeRow).getByText("Sep 23 · in 19d")).toBeInTheDocument();
    // No anchor stored: the cell stays blank rather than guessing a date.
    const codexRow = screen.getByRole("row", { name: /Codex Pro/ });
    expect(codexRow.querySelector(".dashboard-renews-cell")?.textContent).toBe(
      "—",
    );
  } finally {
    vi.useRealTimers();
  }
});
