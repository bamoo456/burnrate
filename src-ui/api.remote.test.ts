import { afterEach, expect, test, vi } from "vitest";

/** Load `api.ts` fresh with the marker `src/server.rs` injects when the page is
 *  served over the LAN, so module-scoped `isRemote` picks it up. */
async function importRemoteApi() {
  vi.resetModules();
  (window as unknown as Record<string, unknown>).__BURNRATE_REMOTE__ = 1;
  return import("./api");
}

afterEach(() => {
  delete (window as unknown as Record<string, unknown>).__BURNRATE_REMOTE__;
  vi.unstubAllGlobals();
  vi.useRealTimers();
  vi.resetModules();
});

test("reads the dashboard over HTTP instead of returning mock data", async () => {
  const payload = {
    accounts: [],
    snapshots: [],
    traySummary: {},
    settings: {},
  };
  const fetchMock = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => payload,
  });
  vi.stubGlobal("fetch", fetchMock);

  const api = await importRemoteApi();
  expect(api.isRemote).toBe(true);
  await expect(api.loadDashboard()).resolves.toEqual(payload);
  expect(fetchMock).toHaveBeenCalledWith("/api/dashboard", {
    credentials: "same-origin",
  });
});

test("surfaces an HTTP failure instead of silently falling back", async () => {
  vi.stubGlobal(
    "fetch",
    vi
      .fn()
      .mockResolvedValue({ ok: false, status: 401, json: async () => ({}) }),
  );

  const api = await importRemoteApi();
  await expect(api.loadDashboard()).rejects.toThrow("401");
});

test("polls the dashboard in place of the missing push channel", async () => {
  vi.useFakeTimers();
  const payload = {
    accounts: [],
    snapshots: [],
    traySummary: {},
    settings: {},
  };
  const fetchMock = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    json: async () => payload,
  });
  vi.stubGlobal("fetch", fetchMock);

  const api = await importRemoteApi();
  const handler = vi.fn();
  const stop = await api.onDashboardUpdated(handler);

  await vi.advanceTimersByTimeAsync(60_000);
  expect(handler).toHaveBeenCalledWith(payload);

  stop();
  await vi.advanceTimersByTimeAsync(120_000);
  expect(handler).toHaveBeenCalledTimes(1);
});
