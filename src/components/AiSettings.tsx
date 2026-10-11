import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorMessage, type AiPreferences, type ProcessingMode } from "../types";
export const DEFAULT_AI: AiPreferences = { mode: "local", enabled: false, cloudConsent: false, provider: "openAi", model: "gpt-4.1-mini" };
export function AiSettings({ value = DEFAULT_AI, disabled, onSave }: {
    value?: AiPreferences; disabled: boolean; onSave: (preferences: AiPreferences) => Promise<void>;
}) {
    const [draft, setDraft] = useState(value);
    const [hasKey, setHasKey] = useState<boolean | null>(null);
    const [busy, setBusy] = useState(false);
    const [message, setMessage] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const keyInput = useRef<HTMLInputElement>(null);
    useEffect(() => { setDraft(value); }, [value.mode, value.enabled, value.cloudConsent, value.provider, value.model]);
    useEffect(() => {
        let active = true;
        void invoke<boolean>("has_ai_credential").then(found => { if (active) setHasKey(found); })
            .catch(() => { if (active) setError("Windows credential storage is unavailable."); });
        return () => { active = false; if (keyInput.current) keyInput.current.value = ""; };
    }, []);
    function edit(next: AiPreferences) { setDraft(next); setMessage(null); setError(null); }
    async function save() {
        if (disabled || busy) return;
        setBusy(true); setError(null); setMessage(null);
        try { await onSave(draft); setMessage("Analysis preferences saved."); }
        catch (failure) { setError(errorMessage(failure)); }
        finally { setBusy(false); }
    }
    async function credential(action: "save" | "remove") {
        if (disabled || busy) return;
        setBusy(true); setError(null); setMessage(null);
        try {
            if (action === "save") {
                const pending = invoke("set_ai_credential", { credential: keyInput.current?.value ?? "" });
                if (keyInput.current) keyInput.current.value = "";
                await pending;
            } else { await invoke("delete_ai_credential"); }
            setHasKey(action === "save");
            setMessage(action === "save" ? "API key saved in Windows Credential Manager." : "API key removed.");
        } catch { setError("The API key could not be updated in Windows Credential Manager."); }
        finally { if (keyInput.current) keyInput.current.value = ""; setBusy(false); }
    }
    const cloud = draft.mode !== "local";
    const validModel = /^[a-zA-Z0-9._-]{1,80}$/.test(draft.model);
    return <fieldset className="naming-settings" disabled={disabled || busy}>
        <legend>Screenshot analysis</legend>
        <label htmlFor="analysis-mode">Analysis mode</label>
        <select id="analysis-mode" value={draft.mode} onChange={event => {
            const mode = event.target.value as ProcessingMode;
            edit({ ...draft, mode, enabled: mode !== "local", cloudConsent: mode === "local" ? false : draft.cloudConsent });
        }}>
            <option value="local">Local</option><option value="ai">AI</option><option value="hybrid">Hybrid</option>
        </select>
        <p className="muted">Local keeps screenshots on this device. AI sends images for analysis. Hybrid tries local naming first and sends images only when local information is insufficient.</p>
        <label htmlFor="analysis-provider">AI provider</label>
        <select id="analysis-provider" value={draft.provider} onChange={() => {}}><option value="openAi">OpenAI</option></select>
        <label htmlFor="analysis-model">Vision model</label>
        <input id="analysis-model" value={draft.model} maxLength={80} onChange={event => edit({ ...draft, model: event.target.value })} />
        <p className="muted">AI and Hybrid may send screenshot image data to OpenAI and incur API charges. Provider retention policies apply. Avoid enabling cloud analysis for sensitive screenshots.</p>
        {cloud ? <label><input type="checkbox" checked={draft.cloudConsent} onChange={event => edit({ ...draft, cloudConsent: event.target.checked })} /> I allow screenshot images to be sent to OpenAI for analysis.</label> : null}
        <button type="button" disabled={disabled || busy || !validModel || (cloud && !draft.cloudConsent)} onClick={() => void save()}>Save analysis preferences</button>
        <label htmlFor="analysis-key">OpenAI API key</label>
        <input id="analysis-key" ref={keyInput} type="password" autoComplete="off" maxLength={2048} />
        <p className="muted">{hasKey === null ? "Checking credential storage…" : hasKey ? "API key saved." : "No API key saved."} Keys are kept in Windows Credential Manager. Saving a key does not enable cloud analysis.</p>
        <button type="button" onClick={() => void credential("save")}>Save API key</button>
        <button type="button" disabled={disabled || busy || hasKey === false} onClick={() => void credential("remove")}>Remove API key</button>
        {error ? <p role="alert" className="error">{error}</p> : null}
        {message ? <p role="status">{message}</p> : null}
    </fieldset>;
}
