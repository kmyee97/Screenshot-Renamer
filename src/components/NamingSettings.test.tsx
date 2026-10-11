import { fireEvent, render, screen, waitFor, act } from "@testing-library/react";
import { vi, test, expect, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { NamingSettings } from "./NamingSettings";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const naming = { preset: "minimal" as const, template: null, maxStemLength: 120, separator: "-" as const };
beforeEach(() => { vi.mocked(invoke).mockImplementation(async (_command, args) => {
  const rules = (args as { rules: { template: string | null; preset: string } }).rules;
  if (rules.template === "{bad}") throw "Unknown token: bad";
  return rules.template ? "code-main.rs.png" : `${rules.preset}-example.png`;
}); });
test("restores saved selection, previews, saves valid templates and rejects invalid ones", async () => {
  const save = vi.fn().mockResolvedValue(undefined);
  render(<NamingSettings value={naming} disabled={false} onSave={save} />);
  expect(screen.getByLabelText("Naming preset")).toHaveValue("minimal");
  expect(await screen.findByText("minimal-example.png")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Custom template"), { target: { value: "{app}-{topic}" } });
  await screen.findByText("code-main.rs.png");
  fireEvent.click(screen.getByRole("button", { name: "Save naming preferences" }));
  await waitFor(() => expect(save).toHaveBeenCalledWith({ ...naming, template: "{app}-{topic}" }));
  fireEvent.change(screen.getByLabelText("Custom template"), { target: { value: "{bad}" } });
  expect(await screen.findByRole("alert")).toHaveTextContent("Unknown token");
  expect(screen.getByRole("button", { name: "Save naming preferences" })).toBeDisabled();
  expect(save).toHaveBeenCalledTimes(1);
});
test("a preset clears overrides and a stale preview cannot enable saving", async () => {
  let resolveOld: (value: string) => void = () => {};
  vi.mocked(invoke).mockImplementation(async (_command, args) => {
    const rules = (args as { rules: { template: string | null } }).rules;
    if (rules.template === "{app}") return new Promise<string>(resolve => { resolveOld = resolve; });
    if (rules.template === "{bad}") throw "Unknown token";
    return "preset.png";
  });
  render(<NamingSettings value={{ ...naming, template: "{app}" }} disabled={false} onSave={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Custom template"), { target: { value: "{bad}" } });
  await screen.findByRole("alert");
  await act(async () => resolveOld("stale.png"));
  expect(screen.queryByText("stale.png")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save naming preferences" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Naming preset"), { target: { value: "coding" } });
  await screen.findByText("preset.png");
  expect(screen.getByLabelText("Custom template")).toHaveValue("");
});
