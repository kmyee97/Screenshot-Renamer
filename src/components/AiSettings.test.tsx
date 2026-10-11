import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi, test, expect, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AiSettings, DEFAULT_AI } from "./AiSettings";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => { vi.mocked(invoke).mockResolvedValue(false); });
test("requires explicit consent before saving cloud mode and can return to Local", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    render(<AiSettings disabled={false} onSave={save} />);
    await screen.findByText(/No API key saved/);
    expect(screen.getByLabelText("Analysis mode")).toHaveValue("local");
    fireEvent.change(screen.getByLabelText("Analysis mode"), { target: { value: "hybrid" } });
    expect(screen.getByRole("button", { name: "Save analysis preferences" })).toBeDisabled();
    fireEvent.click(screen.getByLabelText(/I allow screenshot images/));
    fireEvent.click(screen.getByRole("button", { name: "Save analysis preferences" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith({ ...DEFAULT_AI, mode: "hybrid", enabled: true, cloudConsent: true }));
    fireEvent.change(screen.getByLabelText("Analysis mode"), { target: { value: "local" } });
    fireEvent.click(screen.getByRole("button", { name: "Save analysis preferences" }));
    await waitFor(() => expect(save).toHaveBeenLastCalledWith({ ...DEFAULT_AI, mode: "local", enabled: false, cloudConsent: false }));
});
test("key is only submitted to protected storage and cleared after entry", async () => {
    render(<AiSettings value={DEFAULT_AI} disabled={false} onSave={vi.fn()} />);
    await screen.findByText(/No API key saved/);
    fireEvent.change(screen.getByLabelText("OpenAI API key"), { target: { value: "test-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Save API key" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("set_ai_credential", { credential: "test-secret" }));
    expect(screen.getByLabelText("OpenAI API key")).toHaveValue("");
});
