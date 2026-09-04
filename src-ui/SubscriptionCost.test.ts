import { expect, test } from "vitest";
import {
  formatMonthlyCostUsd,
  formatRenewal,
  nextRenewalDate,
  totalMonthlySubscriptionCost,
} from "./format";
import type { AccountView } from "./types";

function account(
  overrides: Partial<AccountView> & Pick<AccountView, "id">,
): AccountView {
  return {
    provider: "claude-code",
    label: "Claude",
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

test("formats whole and fractional monthly costs without forced cents", () => {
  expect(formatMonthlyCostUsd(20)).toBe("$20/mo");
  expect(formatMonthlyCostUsd(100)).toBe("$100/mo");
  expect(formatMonthlyCostUsd(9.99)).toBe("$9.99/mo");
  expect(formatMonthlyCostUsd(119.99)).toBe("$119.99/mo");
});

test("total subscription cost sums every account, disabled included", () => {
  const accounts = [
    account({ id: "a", subscriptionCostUsd: 100 }),
    account({ id: "b", enabled: false, subscriptionCostUsd: 19.99 }),
    account({ id: "c", subscriptionCostUsd: null }),
    account({ id: "d" }),
  ];

  expect(totalMonthlySubscriptionCost(accounts)).toBeCloseTo(119.99);
  expect(totalMonthlySubscriptionCost([])).toBe(0);
});

test("rolls the renewal anchor forward to the next occurrence", () => {
  // Anchor already in the past: advance to this month, then the next.
  expect(nextRenewalDate("2026-01-07", new Date(2026, 8, 4))).toEqual(
    new Date(2026, 8, 7),
  );
  expect(nextRenewalDate("2026-01-07", new Date(2026, 8, 7))).toEqual(
    new Date(2026, 8, 7),
  );
  expect(nextRenewalDate("2026-01-07", new Date(2026, 8, 8))).toEqual(
    new Date(2026, 9, 7),
  );
  // Anchor in the future is already the next occurrence.
  expect(nextRenewalDate("2026-12-01", new Date(2026, 8, 4))).toEqual(
    new Date(2026, 11, 1),
  );
});

test("clamps a month-end anchor to short months", () => {
  expect(nextRenewalDate("2026-01-31", new Date(2026, 1, 1))).toEqual(
    new Date(2026, 1, 28),
  );
  // Clamping is per-month, not sticky: March still renews on the 31st.
  expect(nextRenewalDate("2026-01-31", new Date(2026, 2, 1))).toEqual(
    new Date(2026, 2, 31),
  );
});

test("rejects a missing or malformed renewal anchor", () => {
  expect(nextRenewalDate(null, new Date(2026, 8, 4))).toBeNull();
  expect(nextRenewalDate("", new Date(2026, 8, 4))).toBeNull();
  expect(nextRenewalDate("not-a-date", new Date(2026, 8, 4))).toBeNull();
  expect(nextRenewalDate("2026-13-01", new Date(2026, 8, 4))).toBeNull();
});

test("describes the next renewal relative to today", () => {
  expect(formatRenewal("2026-09-23", new Date(2026, 8, 4))).toBe(
    "Sep 23 · in 19d",
  );
  expect(formatRenewal("2026-09-04", new Date(2026, 8, 4))).toBe(
    "Sep 4 · today",
  );
  expect(formatRenewal("2026-09-05", new Date(2026, 8, 4))).toBe(
    "Sep 5 · tomorrow",
  );
  expect(formatRenewal(null, new Date(2026, 8, 4))).toBe("—");
});
