import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";

afterEach(() => cleanup());

test("adds OpenCode Go as an account-managed API key provider", async () => {
  const user = userEvent.setup();
  render(<App />);

  await screen.findByRole("heading", { name: "Accounts" });
  await user.click(screen.getByTitle("Add account"));
  await user.click(screen.getByRole("menuitem", { name: "OpenCode Go" }));

  expect(screen.getByLabelText("Label")).toHaveValue("OpenCode Go");
  expect(screen.getByLabelText("API Key")).toBeInTheDocument();
  expect(screen.queryByLabelText("Endpoint")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("menuitem", { name: /Sign in with browser/i }),
  ).not.toBeInTheDocument();
});
