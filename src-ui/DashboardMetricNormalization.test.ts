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

  expect(
    dashboardWindowBucket(snapshot("opencode-go", [rolling]), "5-hour")?.id,
  ).toBe("rolling");
  expect(
    dashboardWindowBucket(snapshot("runpod", [rolling]), "5-hour"),
  ).toBeNull();
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

test("Antigravity's Gemini pair wins the dashboard columns over Claude + GPT", () => {
  const bucket = (
    id: string,
    label: string,
    window: string,
    remaining: number,
  ) => ({
    id,
    label,
    window,
    used: 100 - remaining,
    limit: 100,
    remaining,
    unit: "%",
    resetAt: null,
    status: "healthy" as const,
  });

  // All four buckets carry a real limit, so only the label match separates
  // them — the Claude + GPT pair must not take a column from Gemini.
  const value = snapshot("antigravity", [
    bucket("3p-5h", "Claude + GPT 5-hour", "5-hour", 100),
    bucket("3p-weekly", "Claude + GPT Weekly", "weekly", 100),
    bucket("gemini-5h", "5-hour", "5-hour", 62),
    bucket("gemini-weekly", "Weekly", "weekly", 59),
  ]);

  expect(dashboardWindowBucket(value, "5-hour")?.id).toBe("gemini-5h");
  expect(dashboardWindowBucket(value, "weekly")?.id).toBe("gemini-weekly");
  // Antigravity has no monthly window; the column must stay empty rather than
  // borrowing a weekly bucket.
  expect(dashboardWindowBucket(value, "monthly")).toBeNull();
  expect(secondaryUsageBuckets(value).map((entry) => entry.id)).toEqual([
    "3p-5h",
    "3p-weekly",
  ]);
});
