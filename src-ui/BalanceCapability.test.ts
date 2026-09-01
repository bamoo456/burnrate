import { describe, expect, test } from "vitest";
import {
  balanceBuckets,
  balanceUnavailableReason,
  displayBuckets,
  formatBalance,
} from "./format";
import type { UsageSnapshot } from "./types";

function snapshot(
  provider: UsageSnapshot["provider"],
  usageBuckets: UsageSnapshot["usageBuckets"],
): UsageSnapshot {
  return {
    accountId: `${provider}-test`,
    provider,
    label: provider,
    status: "healthy",
    subscription: null,
    usageBuckets,
    quota: usageBuckets[0]
      ? {
          used: usageBuckets[0].used,
          limit: usageBuckets[0].limit,
          remaining: usageBuckets[0].remaining,
          unit: usageBuckets[0].unit,
          resetAt: usageBuckets[0].resetAt,
        }
      : null,
    message: null,
    fetchedAt: "2026-09-01T12:00:00Z",
  };
}

describe("balance capability", () => {
  test("treats OpenRouter credits as spendable balance, not quota", () => {
    const value = snapshot("openrouter", [
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
    ]);

    expect(balanceBuckets(value)).toHaveLength(1);
    expect(formatBalance(balanceBuckets(value)[0])).toBe("$17.75");
    expect(displayBuckets(value)).toEqual([]);
  });

  test("separates Runpod wallet balance from burn and 24h spend", () => {
    const value = snapshot("runpod", [
      {
        id: "balance",
        label: "Balance",
        window: null,
        used: 0,
        limit: null,
        remaining: 42.5,
        unit: "USD",
        resetAt: null,
        status: "healthy",
      },
      {
        id: "current-burn",
        label: "Current burn",
        window: null,
        used: 1.25,
        limit: 10,
        remaining: null,
        unit: "USD/hr",
        resetAt: null,
        status: "healthy",
      },
      {
        id: "pods-24h",
        label: "Pods 24h",
        window: "24h",
        used: 3.4,
        limit: null,
        remaining: null,
        unit: "USD",
        resetAt: null,
        status: "healthy",
      },
    ]);

    expect(balanceBuckets(value).map((bucket) => bucket.id)).toEqual(["balance"]);
    expect(displayBuckets(value).map((bucket) => bucket.id)).toEqual([
      "current-burn",
      "pods-24h",
    ]);
  });

  test("states that OpenCode Go Zen balance is not exposed by the API", () => {
    const value = snapshot("opencode-go", [
      {
        id: "rolling",
        label: "5-hour",
        window: "5-hour",
        used: 42,
        limit: 100,
        remaining: 58,
        unit: "%",
        resetAt: "2026-09-01T15:30:00Z",
        status: "healthy",
      },
    ]);

    expect(balanceBuckets(value)).toEqual([]);
    expect(balanceUnavailableReason(value)).toBe(
      "Zen balance unavailable via API",
    );
    expect(displayBuckets(value)).toHaveLength(1);
  });
});
