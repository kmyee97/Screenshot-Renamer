import { render, screen, act } from "@testing-library/react";
import { vi, test, expect } from "vitest";
import { PipelineNotice } from "./PipelineNotice";
const callbacks = vi.hoisted(() => new Map<string, (event: { payload: string }) => void>());
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (name, callback) => { callbacks.set(name, callback); return () => callbacks.delete(name); }) }));
test("shows analysis fallback and naming failures and releases listeners", async () => {
  const view = render(<PipelineNotice />);
  await act(async () => {});
  act(() => callbacks.get("screenshot-analysis-warning")?.({ payload: "OCR unavailable; fallback naming used." }));
  expect(screen.getByRole("status")).toHaveTextContent("fallback naming");
  act(() => callbacks.get("screenshot-naming-failed")?.({ payload: "Invalid naming rule. Screenshot remains intact." }));
  expect(screen.getByRole("alert")).toHaveTextContent("remains intact");
  act(() => callbacks.get("screenshot-analysis-completed")?.({ payload: "Renamed using AI analysis." }));
  expect(screen.getByRole("status")).toHaveTextContent("using AI analysis");
  view.unmount();
  expect(callbacks.size).toBe(0);
});
