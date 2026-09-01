import { expect, test } from "vitest";
import {
  dashboardWindowBucket,
  nextResetBucket,
  secondaryUsageBuckets,
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
    email: null,
    subscription: null,
    usageBuckets,
    quota: null,
    message: null,
    fetchedAt: new Date().toISOString(),
  };
}

test("prefers AWS month total over month-to-date category rows", () => {
  const value = snapshot("aws", [
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
  ]);

  expect(dashboardWindowBucket(value, "monthly")?.id).toBe("aws-mtd");
  expect(secondaryUsageBuckets(value).map((bucket) => bucket.id)).toEqual([
    "aws-category-bedrock",
  ]);
});

test("only treats OpenCode Go rolling as the five-hour alias", () => {
  const rolling = {
    id: "rolling",
    label: "Rolling",
    window: null,
    used: 40,
    limit: 100,
    remaining: 60,
    unit: "%",
    resetAt: null,
    status: "healthy" as const,
  };

  expect(dashboardWindowBucket(snapshot("opencode-go", [rolling]), "5-hour")?.id).toBe(
    "rolling",
  );
  expect(dashboardWindowBucket(snapshot("runpod", [rolling]), "5-hour")).toBeNull();
});

test("next reset ignores expired windows", () => {
  const now = Date.now();
  const value = snapshot("codex", [
    {
      id: "5-hour",
      label: "5-hour",
      window: "5-hour",
      used: 90,
      limit: 100,
      remaining: 10,
      unit: "requests",
      resetAt: new Date(now - 60_000).toISOString(),
      status: "warning",
    },
    {
      id: "weekly",
      label: "Weekly",
      window: "weekly",
      used: 200,
      limit: 500,
      remaining: 300,
      unit: "requests",
      resetAt: new Date(now + 3_600_000).toISOString(),
      status: "healthy",
    },
  ]);

  expect(nextResetBucket(value)?.id).toBe("weekly");
});
