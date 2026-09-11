import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test } from "vitest";
import { App } from "./App";

afterEach(() => cleanup());

test("adds Cursor as a CLI-credential provider with no API key field", async () => {
  const user = userEvent.setup();
  render(<App />);

  await screen.findByRole("heading", { name: "Accounts" });
  await user.click(screen.getByTitle("Add account"));
  await user.click(screen.getByRole("menuitem", { name: "Cursor" }));

  expect(screen.getByLabelText("Label")).toHaveValue("Cursor");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.getByText(/cursor-agent login/)).toBeInTheDocument();
});
