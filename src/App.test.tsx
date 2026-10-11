import { fireEvent, render, screen } from "@testing-library/react";
import { test, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockResolvedValue({ settings: { version: 1, watchedFolder: null, autoRename: false }, warning: null }));

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

test("restores the saved folder and shows nonfatal settings warnings", async () => {
  vi.mocked(invoke).mockResolvedValue({ settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: false }, warning: "Recovered invalid settings." });
  render(<App />);
  expect(await screen.findByText("C:/Screenshots")).toBeInTheDocument();
  expect(screen.getByText("Recovered invalid settings.")).toBeInTheDocument();
});
