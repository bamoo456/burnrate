import { expect, test } from "vitest";
import {
  formatMonthlyCostUsd,
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
