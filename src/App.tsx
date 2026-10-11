import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage, type SettingsSnapshot, type WatcherEvent } from "./types";
import { WatchedFolder } from "./components/WatchedFolder";
import { WatcherControls } from "./components/WatcherControls";
import { RecentRenames } from "./components/RecentRenames";
import "./App.css";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);
  const [toggling, setToggling] = useState(false);
  const watcherEvent = useRef<{ revision: number; value: WatcherEvent | null }>({ revision: 0, value: null });
  function commitResponse(value: SettingsSnapshot, revision: number) {
    const event = watcherEvent.current;
    setSnapshot(event.revision > revision && event.value ? {
      ...value, watcherStatus: event.value.watcherStatus, watcherError: event.value.watcherError,
    } : value);
  }
  async function toggleAutoRename() {
    if (!snapshot || choosing || toggling) return;
    setToggling(true);
    setSettingsError(null);
    const revision = watcherEvent.current.revision;
    try { commitResponse(await invoke<SettingsSnapshot>("set_auto_rename", { enabled: !snapshot.settings.autoRename }), revision); }
    catch (error) { setSettingsError(errorMessage(error)); }
    finally { setToggling(false); }
  }
  async function chooseFolder() {
    if (!snapshot || choosing || toggling) return;
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
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined;
    let latestEvent: WatcherEvent | null = null;
    async function initialize() {
      try {
        const release = await listen<WatcherEvent>("watcher-state-changed", event => {
          watcherEvent.current = { revision: watcherEvent.current.revision + 1, value: event.payload };
          latestEvent = event.payload;
          if (active) setSnapshot(current => current ? { ...current, ...event.payload } : current);
        });
        if (!active) { release(); return; }
        unlisten = release;
        const value = await invoke<SettingsSnapshot>("get_settings");
        if (active) setSnapshot({ ...value, ...(latestEvent ?? {}) });
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
      <WatchedFolder folder={snapshot?.settings.watchedFolder ?? null} onChoose={snapshot ? chooseFolder : undefined} pending={choosing || toggling} />
      <WatcherControls snapshot={snapshot} pending={choosing || toggling} onToggle={toggleAutoRename} />
      <RecentRenames />
      {settingsOpen ? <section className="panel" id="settings-panel" aria-labelledby="settings-heading">
        <div className="section-header"><h2 id="settings-heading">Settings</h2><button onClick={() => setSettingsOpen(false)}>Close settings</button></div>
        <p>Folder: {snapshot?.settings.watchedFolder ?? "Not configured"}</p>
        <p>Auto Rename preference: {snapshot?.settings.autoRename ? "On" : "Off"}</p>
        <p className="muted">Preferences are saved on this device.</p>
      </section> : null}
    </main>
  );
}
export default App;
