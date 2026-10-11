import { fireEvent, render, screen } from "@testing-library/react";
import { test, expect } from "vitest";
import App from "./App";

test("shows the screenshot controls and an honest empty history", () => {
  render(<App />);

  expect(screen.getByRole("heading", { name: "Screenshot Renamer" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Watched folder" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Recent renames" })).toBeInTheDocument();
  expect(screen.getByText("No screenshots renamed yet.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Choose folder" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Undo Last" })).toBeDisabled();
});

test("opens and closes settings without losing the main controls", () => {
  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Close settings" }));
  expect(screen.queryByRole("heading", { name: "Settings" })).not.toBeInTheDocument();
});
