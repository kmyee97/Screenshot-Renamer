import { render, screen } from "@testing-library/react";
import { test, expect, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

test("renders the primary interface without starting Tauri", () => {
  render(<App />);

  expect(
    screen.getByRole("heading", { name: "Welcome to Tauri + React" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Greet" })).toBeEnabled();
});
