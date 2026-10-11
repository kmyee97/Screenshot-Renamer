import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage, type SettingsSnapshot, type WatcherEvent, type NamingRules } from "./types";
import { WatchedFolder } from "./components/WatchedFolder";
import { WatcherControls } from "./components/WatcherControls";
import { RecentRenames } from "./components/RecentRenames";
import { PipelineNotice } from "./components/PipelineNotice";
import { AnalysisFailures } from "./components/AnalysisFailures";
import { NamingSettings } from "./components/NamingSettings";
import { AiSettings } from "./components/AiSettings";
import type { AiPreferences } from "./types";
import "./App.css";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);
  const [toggling, setToggling] = useState(false);
  const [savingNaming, setSavingNaming] = useState(false);
  const watcherEvent = useRef<{ revision: number; value: WatcherEvent | null }>({ revision: 0, value: null });
  function commitResponse(value: SettingsSnapshot, revision: number) {
    const event = watcherEvent.current;
    if (value.revision !== undefined) {
      setSnapshot(current => current?.revision !== undefined && current.revision > value.revision! ? current : value);
      return;
    }
    setSnapshot(event.revision > revision && event.value ? {
      ...value, ...event.value,
    } : value);
  }
  async function toggleAutoRename() {
    if (!snapshot || choosing || toggling || savingNaming) return;
    setToggling(true);
    setSettingsError(null);
    const revision = watcherEvent.current.revision;
    try { commitResponse(await invoke<SettingsSnapshot>("set_auto_rename", { enabled: !snapshot.settings.autoRename }), revision); }
    catch (error) { setSettingsError(errorMessage(error)); }
    finally { setToggling(false); }
  }
  async function chooseFolder() {
    if (!snapshot || choosing || toggling || savingNaming) return;
    setChoosing(true);
    setSettingsError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Choose screenshot folder", defaultPath: snapshot.settings.watchedFolder ?? undefined });
      if (selected !== null) {
        const revision = watcherEvent.current.revision;
        commitResponse(await invoke<SettingsSnapshot>("update_settings", { settings: { ...snapshot.settings, watchedFolder: selected } }), revision);
      }
    } catch (error) { setSettingsError(errorMessage(error)); }
    finally { setChoosing(false); }
  }
  async function saveNaming(rules: NamingRules) {
    if (!snapshot || choosing || toggling || savingNaming) throw "Wait for the current settings operation to finish.";
    setSavingNaming(true);
    const revision = watcherEvent.current.revision;
    try { commitResponse(await invoke<SettingsSnapshot>("update_settings", { settings: { ...snapshot.settings, naming: rules } }), revision); }
    finally { setSavingNaming(false); }
  }
  async function saveAnalysis(ai: AiPreferences) {
    if (!snapshot || choosing || toggling || savingNaming) throw "Wait for the current settings operation to finish.";
    setSavingNaming(true);
    const revision = watcherEvent.current.revision;
    try { commitResponse(await invoke<SettingsSnapshot>("update_settings", { settings: { ...snapshot.settings, ai } }), revision); }
    finally { setSavingNaming(false); }
  }
  useEffect(() => {
    let active = true;
    const releases: UnlistenFn[] = [];
    async function attach() {
      for (const [name, callback] of [
        ["open-settings", () => setSettingsOpen(true)],
        ["desktop-error", (event: { payload: string }) => setSettingsError(event.payload)],
      ] as const) {
        const release = await listen(name, callback as (event: { payload: unknown }) => void);
        if (!active) release(); else releases.push(release);
      }
    }
    void attach().catch(() => { if (active) setSettingsError("Could not connect tray controls. Reopen the app to retry."); });
    return () => { active = false; releases.forEach(release => release()); };
  }, []);
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    async function initialize() {
      try {
        const release = await listen<WatcherEvent>("watcher-state-changed", event => {
          if (event.payload.revision !== undefined && watcherEvent.current.value?.revision !== undefined && event.payload.revision < watcherEvent.current.value.revision) return;
          watcherEvent.current = { revision: watcherEvent.current.revision + 1, value: event.payload };
          if (active) setSnapshot(current => current && !(current.revision !== undefined && event.payload.revision !== undefined && current.revision > event.payload.revision) ? { ...current, ...event.payload } : current);
        });
        if (!active) { release(); return; }
        unlisten = release;
        const value = await invoke<SettingsSnapshot>("get_settings");
        if (active) commitResponse(value, 0);
      } catch (error) { if (active) setSettingsError(errorMessage(error)); }
    }
    void initialize();
    return () => { active = false; unlisten?.(); };
  }, []);
  return (
    <main className="app-shell">
      <header className="app-header">
        <div><p className="eyebrow">YOUR SCREENSHOTS, ORGANIZED</p><h1>Screenshot Renamer</h1></div>
        <button aria-expanded={settingsOpen} aria-controls="settings-panel" onClick={() => setSettingsOpen(!settingsOpen)}>Settings</button>
      </header>
      {settingsError ? <p role="alert" className="error">{settingsError}</p> : null}
      {snapshot?.warning ? <p role="status" className="muted">{snapshot.warning}</p> : null}
      <WatchedFolder folder={snapshot?.settings.watchedFolder ?? null} onChoose={snapshot ? chooseFolder : undefined} pending={choosing || toggling || savingNaming} />
      <WatcherControls snapshot={snapshot} pending={choosing || toggling || savingNaming} onToggle={toggleAutoRename} />
      <RecentRenames />
      <PipelineNotice />
      <AnalysisFailures />
      <p className="muted">Analysis mode: {snapshot?.settings.ai?.mode ?? "local"}{snapshot?.settings.ai?.mode && snapshot.settings.ai.mode !== "local" ? " · OpenAI may receive screenshot images" : " · On this device"}</p>
      {settingsOpen ? <section className="panel" id="settings-panel" aria-labelledby="settings-heading">
        <div className="section-header"><h2 id="settings-heading">Settings</h2><button onClick={() => setSettingsOpen(false)}>Close settings</button></div>
        <p>Folder: {snapshot?.settings.watchedFolder ?? "Not configured"}</p>
        <p>Auto Rename preference: {snapshot?.settings.autoRename ? "On" : "Off"}</p>
        {snapshot ? <NamingSettings value={snapshot.settings.naming} disabled={choosing || toggling || savingNaming} onSave={saveNaming} /> : null}
        {snapshot ? <AiSettings value={snapshot.settings.ai} disabled={choosing || toggling || savingNaming} onSave={saveAnalysis} /> : null}
        <p className="muted">Preferences are saved on this device.</p>
      </section> : null}
    </main>
  );
}
export default App;
