import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { test, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import App from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
beforeEach(async () => {
  vi.mocked(invoke).mockImplementation(async command => command === "list_recent_history" ? [] : { settings: { version: 1, watchedFolder: null, autoRename: false }, warning: null });
  const { listen } = await import("@tauri-apps/api/event");
  vi.mocked(listen).mockResolvedValue(() => {});
});

test("shows the screenshot controls and an honest empty history", async () => {
  render(<App />);

  expect(screen.getByRole("heading", { name: "Screenshot Renamer" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Watched folder" })).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Recent renames" })).toBeInTheDocument();
  expect(await screen.findByText("No screenshots renamed yet.")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("button", { name: "Choose folder" })).toBeEnabled());
  expect(screen.getByRole("button", { name: "Undo Last" })).toBeDisabled();
});

test("commits a selected folder and leaves it unchanged when cancelled", async () => {
  let folder: string | null = "C:/Old";
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "list_recent_history") return [];
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
    if (command === "list_recent_history") return [];
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
  vi.mocked(invoke).mockImplementation(async command => command === "list_recent_history" ? [] : { settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: false }, warning: "Recovered invalid settings." });
  render(<App />);
  expect(await screen.findByText("C:/Screenshots")).toBeInTheDocument();
  expect(screen.getByText("Recovered invalid settings.")).toBeInTheDocument();
});

test("toggle shows committed watching state and disables pending requests", async () => {
  let release: (value: unknown) => void = () => {};
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === "list_recent_history") return [];
    if (command === "set_auto_rename") return new Promise(resolve => { release = resolve; });
    return { settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: false }, warning: null, watcherStatus: "Paused", watcherError: null };
  });
  render(<App />);
  await screen.findByText("C:/Screenshots");
  const toggle = screen.getByRole("checkbox", { name: "Auto Rename" });
  fireEvent.click(toggle);
  expect(toggle).toBeDisabled();
  release({ settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: true }, warning: null, watcherStatus: "Watching", watcherError: null });
  expect(await screen.findByText("Watching")).toBeInTheDocument();
  expect(toggle).toBeChecked();
  expect(toggle).toBeEnabled();
});

test("watcher failure events show Error and listener is released", async () => {
  const { listen } = await import("@tauri-apps/api/event");
  const unlisten = vi.fn();
  let handler: (event: { payload: { watcherStatus: string; watcherError: string } }) => void = () => {};
  vi.mocked(listen).mockImplementation(async (name, callback) => { if (name !== "watcher-state-changed") return () => {}; handler = callback as typeof handler; return unlisten; });
  const { unmount } = render(<App />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Choose folder" })).toBeEnabled());
  const { act } = await import("@testing-library/react");
  act(() => handler({ payload: { watcherStatus: "Error", watcherError: "Folder disappeared." } }));
  expect(screen.getByText("Error")).toBeInTheDocument();
  expect(screen.getByText("Folder disappeared.")).toBeInTheDocument();
  unmount();
  expect(unlisten).toHaveBeenCalledOnce();
});

test("a delayed command response cannot erase a newer watcher failure", async () => {
  const { listen } = await import("@tauri-apps/api/event");
  const { act } = await import("@testing-library/react");
  let handler: (event: { payload: { watcherStatus: string; watcherError: string } }) => void = () => {};
  let release: (value: unknown) => void = () => {};
  vi.mocked(listen).mockImplementation(async (name, callback) => { if (name === "watcher-state-changed") handler = callback as typeof handler; return () => {}; });
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === "list_recent_history") return [];
    if (command === "set_auto_rename") return new Promise(resolve => { release = resolve; });
    return { settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: false }, watcherStatus: "Paused", watcherError: null, warning: null };
  });
  render(<App />);
  await screen.findByText("C:/Screenshots");
  fireEvent.click(screen.getByRole("checkbox", { name: "Auto Rename" }));
  act(() => handler({ payload: { watcherStatus: "Error", watcherError: "Folder disappeared." } }));
  release({ settings: { version: 1, watchedFolder: "C:/Screenshots", autoRename: true }, watcherStatus: "Watching", watcherError: null, warning: null });
  await waitFor(() => expect(screen.getByRole("checkbox", { name: "Auto Rename" })).toBeEnabled());
  expect(screen.getByText("Error")).toBeInTheDocument();
  expect(screen.getByText("Folder disappeared.")).toBeInTheDocument();
});
