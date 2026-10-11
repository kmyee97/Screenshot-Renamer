import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { errorMessage, type NamingRules, type NamingPreset } from "../types";
export const DEFAULT_NAMING: NamingRules = { preset: "descriptive", template: null, maxStemLength: 120, separator: "-" };
const PRESETS: { value: NamingPreset; label: string }[] = [
  { value: "descriptive", label: "Descriptive" }, { value: "minimal", label: "Minimal" },
  { value: "coding", label: "Coding" }, { value: "workResearch", label: "Work/Research" },
  { value: "receiptsOrders", label: "Receipts/Orders" },
];
export function NamingSettings({ value = DEFAULT_NAMING, disabled, onSave }: {
  value?: NamingRules; disabled: boolean; onSave: (rules: NamingRules) => Promise<void>;
}) {
  const [draft, setDraft] = useState(value);
  const [preview, setPreview] = useState<string | null>(null);
  const [validation, setValidation] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    setDraft({ preset: value.preset, template: value.template, maxStemLength: value.maxStemLength, separator: value.separator });
  }, [value.preset, value.template, value.maxStemLength, value.separator]);
  useEffect(() => {
    let active = true;
    setPreview(null); setValidation(null);
    void invoke<string>("preview_naming", { rules: draft }).then(result => {
      if (active) setPreview(result);
    }).catch(error => { if (active) setValidation(errorMessage(error)); });
    return () => { active = false; };
  }, [draft]);
  function edit(next: NamingRules) {
    setPreview(null); setValidation(null); setSaved(false); setSaveError(null); setDraft(next);
  }
  async function save() {
    if (disabled || saving || preview === null || validation) return;
    setSaving(true); setSaveError(null); setSaved(false);
    try { await onSave(draft); setSaved(true); }
    catch (error) { setSaveError(errorMessage(error)); }
    finally { setSaving(false); }
  }
  return <fieldset className="naming-settings" disabled={disabled || saving}>
    <legend>Filename style</legend>
    <label htmlFor="naming-preset">Naming preset</label>
    <select id="naming-preset" value={draft.preset} onChange={event => edit({ ...draft, preset: event.target.value as NamingPreset, template: null })}>
      {PRESETS.map(preset => <option key={preset.value} value={preset.value}>{preset.label}</option>)}
    </select>
    <label htmlFor="naming-template">Custom template</label>
    <input id="naming-template" value={draft.template ?? ""} placeholder="Use the selected preset" maxLength={512}
      aria-describedby="naming-template-help" aria-invalid={validation !== null}
      onChange={event => edit({ ...draft, template: event.target.value === "" ? null : event.target.value })} />
    <p className="muted" id="naming-template-help">Optional. Tokens: {"{app}, {topic}, {detail}, {timestamp}"}. Double braces keep literal braces. Missing fields are skipped.</p>
    <p className="filename-preview">Example: <output aria-label="Filename preview">{preview ?? (validation ? "Unavailable" : "Checking…")}</output></p>
    {validation ? <p role="alert" className="error">{validation}</p> : null}
    {saveError ? <p role="alert" className="error">{saveError}</p> : null}
    <button type="button" onClick={() => void save()} disabled={disabled || saving || preview === null || validation !== null}>Save naming preferences</button>
    {saved ? <p role="status">Naming preferences saved.</p> : null}
  </fieldset>;
}
