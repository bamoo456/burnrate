import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";

afterEach(() => cleanup());

test("presents managed provider usage as a single dashboard", async () => {
  render(<App />);

  expect(
    await screen.findByRole("heading", { name: "Usage Dashboard" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("region", { name: "Dashboard overview" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Live usage" })).toBeInTheDocument();
  expect(screen.getAllByText(/^Updated · just now$/).length).toBeGreaterThan(1);
  expect(screen.getByText(/^AWS cost data · just now$/)).toBeInTheDocument();
});
