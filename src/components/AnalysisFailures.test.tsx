import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi, test, expect } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AnalysisFailures } from "./AnalysisFailures";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const callbacks = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (name, callback) => { callbacks.set(name, callback); return () => callbacks.delete(name); }) }));
const failure = { id: "analysis-1", sourcePath: "C:/Shots/source.png", message: "AI timed out. The screenshot has not been renamed.", retryable: true };
test("shows intact source and retries with only the reserved ID, disabling repeated clicks", async () => {
  let finish!: () => void;
  vi.mocked(invoke).mockImplementation(async command => command === "list_analysis_failures" ? [failure] : new Promise<void>(resolve => { finish = resolve; }));
  const view = render(<AnalysisFailures />);
  fireEvent.click(await screen.findByRole("button", { name: "Retry analysis" }));
  expect(screen.getByRole("button", { name: "Retrying…" })).toBeDisabled();
  expect(invoke).toHaveBeenCalledWith("retry_analysis", { id: "analysis-1" });
  await act(async () => finish());
  await waitFor(() => expect(screen.queryByText(failure.sourcePath)).not.toBeInTheDocument());
  view.unmount(); expect(callbacks.size).toBe(0);
});
test("a replacement source disables retry and a rejected request shows a safe error", async () => {
  vi.mocked(invoke).mockImplementation(async command => { if (command === "list_analysis_failures") return [failure]; throw "Original screenshot changed."; });
  const view = render(<AnalysisFailures />);
  fireEvent.click(await screen.findByRole("button", { name: "Retry analysis" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Original screenshot changed.");
  act(() => callbacks.get("analysis-failures-changed")?.({ payload: [{ ...failure, retryable: false }] }));
  expect(screen.getByRole("button", { name: "Retry analysis" })).toBeDisabled();
  view.unmount();
});
