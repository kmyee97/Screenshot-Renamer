import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { test, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockResolvedValue({ settings: { version: 1, watchedFolder: null, autoRename: false }, warning: null }));

test("shows the screenshot controls and an honest empty history", async () => {
  render(<App />);

  expect(screen.getByRole("heading", { name: "Screenshot Renamer" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Watched folder" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Recent renames" })).toBeInTheDocument();
  expect(screen.getByText("No screenshots renamed yet.")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("button", { name: "Choose folder" })).toBeEnabled());
  expect(screen.getByRole("button", { name: "Undo Last" })).toBeDisabled();
});

test("commits a selected folder and leaves it unchanged when cancelled", async () => {
  let folder: string | null = "C:/Old";
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "update_settings") folder = (args as { settings: { watchedFolder: string } }).settings.watchedFolder;
    return { settings: { version: 1, watchedFolder: folder, autoRename: false }, warning: null };
  });
  vi.mocked(open).mockResolvedValueOnce("C:/New").mockResolvedValueOnce(null);
  render(<App />);
  await screen.findByText("C:/Old");
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  expect(await screen.findByText("C:/New")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Choose folder" })).toBeEnabled());
  expect(screen.getByText("C:/New")).toBeInTheDocument();
});

test("a rejected folder keeps the previous selection and shows the error", async () => {
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === "update_settings") throw "Cannot watch this folder.";
    return { settings: { version: 1, watchedFolder: "C:/Old", autoRename: false }, warning: null };
  });
  vi.mocked(open).mockResolvedValue("C:/Invalid");
  render(<App />);
  await screen.findByText("C:/Old");
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Cannot watch this folder.");
  expect(screen.getByText("C:/Old")).toBeInTheDocument();
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
