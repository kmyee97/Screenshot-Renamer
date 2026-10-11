import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RecentRenames } from "./RecentRenames";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
const old = { id: "old", originalName: "Screenshot.png", newName: "project.png", originalPath: "C:/Screenshot.png", newPath: "C:/project.png", attemptedAtMs: 1000, outcome: { status: "succeeded" }, undoStatus: { status: "notAttempted" } };
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue([]); vi.mocked(listen).mockReset().mockResolvedValue(() => {}); });

test("loads persisted successful renames newest first with names and outcome", async () => {
  vi.mocked(invoke).mockResolvedValue([old, { ...old, id: "new", originalName: "New screenshot.png", newName: "latest.png", attemptedAtMs: 2000 }]);
  render(<RecentRenames />);
  expect(await screen.findByText("latest.png")).toBeInTheDocument();
  expect(screen.getAllByRole("listitem")[0]).toHaveTextContent("New screenshot.png");
  expect(screen.getAllByRole("listitem")[1]).toHaveTextContent("Screenshot.png");
  expect(screen.getAllByText("Renamed")).toHaveLength(2);
  expect(screen.getAllByRole("time")).toHaveLength(2);
});

test("failed history loads allow retry without disabling the rest of the app", async () => {
  vi.mocked(invoke).mockRejectedValueOnce("History database is unavailable.").mockResolvedValueOnce([]);
  render(<RecentRenames />);
  expect(await screen.findByRole("alert")).toHaveTextContent("History database is unavailable.");
  fireEvent.click(screen.getByRole("button", { name: "Refresh history" }));
  expect(await screen.findByText("No screenshots renamed yet.")).toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("live events refresh without duplicates or stale query overwrites and clean up", async () => {
  let handler: () => void = () => {};
  const release = vi.fn();
  vi.mocked(listen).mockImplementation(async (_name, callback) => { handler = () => callback({ event: "screenshot-renamed", id: 1, payload: old }); return release; });
  let finishInitial: (value: unknown) => void = () => {};
  vi.mocked(invoke).mockImplementationOnce(() => new Promise(resolve => { finishInitial = resolve; })).mockResolvedValue([old, old]);
  const { unmount } = render(<RecentRenames />);
  await waitFor(() => expect(invoke).toHaveBeenCalled());
  act(() => handler());
  expect(await screen.findByText("project.png")).toBeInTheDocument();
  act(() => finishInitial([]));
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
  unmount();
  expect(release).toHaveBeenCalledOnce();
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
  await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
