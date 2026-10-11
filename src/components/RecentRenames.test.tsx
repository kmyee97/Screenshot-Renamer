import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RecentRenames } from "./RecentRenames";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const old = { id: "old", originalName: "Screenshot.png", newName: "project.png", originalPath: "C:/Screenshot.png", newPath: "C:/project.png", attemptedAtMs: 1000, outcome: { status: "succeeded" }, undoStatus: { status: "notAttempted" }, canUndo: true, undoReason: null };
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue({ entries: [], undoLast: null }); vi.mocked(listen).mockReset().mockResolvedValue(() => {}); });

test("loads persisted successful renames newest first with names and outcome", async () => {
  vi.mocked(invoke).mockResolvedValue({ entries: [old, { ...old, id: "new", originalName: "New screenshot.png", newName: "latest.png", attemptedAtMs: 2000 }], undoLast: old });
  render(<RecentRenames />);
  expect(await screen.findByText("latest.png")).toBeInTheDocument();
  expect(screen.getAllByRole("listitem")[0]).toHaveTextContent("New screenshot.png");
  expect(screen.getAllByRole("listitem")[1]).toHaveTextContent("Screenshot.png");
  expect(screen.getAllByText("Renamed")).toHaveLength(2);
  expect(screen.getAllByRole("time")).toHaveLength(2);
});

test("failed history loads allow retry without disabling the rest of the app", async () => {
  vi.mocked(invoke).mockRejectedValueOnce("History database is unavailable.").mockResolvedValueOnce({ entries: [], undoLast: null });
  render(<RecentRenames />);
  expect(await screen.findByRole("alert")).toHaveTextContent("History database is unavailable.");
  fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
  expect(await screen.findByText("No screenshots renamed yet.")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("live events refresh without duplicates or stale query overwrites and clean up", async () => {
  let handler: () => void = () => {};
  const release = vi.fn();
  vi.mocked(listen).mockImplementation(async (name, callback) => { if (name === "screenshot-renamed") handler = () => callback({ event: "screenshot-renamed", id: 1, payload: old }); return release; });
  let finishInitial: (value: unknown) => void = () => {};
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { finishInitial = resolve; })).mockResolvedValue({ entries: [old, old], undoLast: old });
  const { unmount } = render(<RecentRenames />);
  await waitFor(() => expect(invoke).toHaveBeenCalled());
  act(() => handler());
  expect(await screen.findByText("project.png")).toBeInTheDocument();
  act(() => finishInitial({ entries: [], undoLast: null }));
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
  unmount();
  expect(release).toHaveBeenCalledTimes(2);
});

test("a listener resolving after unmount is immediately released", async () => {
  let finish: (release: () => void) => void = () => {};
  const release = vi.fn();
  vi.mocked(listen).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const { unmount } = render(<RecentRenames />);
  unmount();
  await act(async () => finish(release));
  expect(release).toHaveBeenCalledOnce();
  expect(invoke).not.toHaveBeenCalled();
});

test("subscription failures remain visible and refresh reconnects live updates", async () => {
  vi.mocked(listen).mockRejectedValueOnce("Live history updates disconnected.").mockResolvedValueOnce(() => {});
  render(<RecentRenames />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Refresh history" })).toBeEnabled());
  expect(screen.getByRole("alert")).toHaveTextContent("Live history updates disconnected.");
  fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
  await waitFor(() => expect(listen).toHaveBeenCalledTimes(3));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("Undo Last targets the eligible backend ID and disables actions until refreshed", async () => {
  let complete: (value: unknown) => void = () => {};
  let undone = false;
  const missing = { ...old, id: "missing", originalName: "Missing.png", newName: "missing.png", attemptedAtMs: 2000, canUndo: false, undoReason: "Renamed screenshot is missing." };
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "undo_history") {
      expect(args).toEqual({ id: "old" });
      return new Promise(resolve => { complete = value => { undone = true; resolve(value); }; });
    }
    return { entries: [missing, { ...old, canUndo: !undone, undoStatus: { status: undone ? "succeeded" : "notAttempted" } }], undoLast: undone ? null : old };
  });
  render(<RecentRenames />);
  await screen.findByText("project.png");
  expect(screen.getByRole("button", { name: "Undo rename of Missing.png" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Undo Last" }));
  expect(screen.getByRole("button", { name: "Undo Last" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Undo rename of Screenshot.png" })).toBeDisabled();
  await act(async () => complete(old));
  expect(await screen.findByText("Undone")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Undo Last" })).toBeDisabled();
  expect(screen.getByText("Restored Screenshot.png.")).toBeInTheDocument();
});

test("per-row undo surfaces typed occupied-path failure and refreshes eligibility", async () => {
  let blocked = false;
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "undo_history") { expect(args).toEqual({ id: "old" }); blocked = true; throw { category: "originalOccupied", message: "Original path is occupied: C:/Screenshot.png" }; }
    return { entries: [{ ...old, canUndo: !blocked, undoReason: blocked ? "Original path is occupied." : null }], undoLast: blocked ? null : old };
  });
  render(<RecentRenames />);
  await screen.findByText("project.png");
  fireEvent.click(screen.getByRole("button", { name: "Undo rename of Screenshot.png" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Original path is occupied: C:/Screenshot.png");
  await waitFor(() => expect(screen.getByRole("button", { name: "Undo rename of Screenshot.png" })).toBeDisabled());
  expect(screen.queryByText("Undone")).not.toBeInTheDocument();
});
