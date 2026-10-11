import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { errorMessage, type SettingsSnapshot } from "./types";
import { WatchedFolder } from "./components/WatchedFolder";
import { WatcherControls } from "./components/WatcherControls";
import { RecentRenames } from "./components/RecentRenames";
import "./App.css";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [snapshot, setSnapshot] = useState<SettingsSnapshot | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);
  async function chooseFolder() {
    if (!snapshot || choosing) return;
    setChoosing(true);
    setSettingsError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Choose screenshot folder", defaultPath: snapshot.settings.watchedFolder ?? undefined });
      if (selected !== null) {
        setSnapshot(await invoke<SettingsSnapshot>("update_settings", { settings: { ...snapshot.settings, watchedFolder: selected } }));
      }
    } catch (error) { setSettingsError(errorMessage(error)); }
    finally { setChoosing(false); }
  }
  useEffect(() => {
    let active = true;
    invoke<SettingsSnapshot>("get_settings").then(value => { if (active) setSnapshot(value); })
      .catch(error => { if (active) setSettingsError(errorMessage(error)); });
    return () => { active = false; };
  }, []);
  return (
    <main className="app-shell">
      <header className="app-header">
        <div><p className="eyebrow">YOUR SCREENSHOTS, ORGANIZED</p><h1>Screenshot Renamer</h1></div>
        <button aria-expanded={settingsOpen} aria-controls="settings-panel" onClick={() => setSettingsOpen(!settingsOpen)}>Settings</button>
      </header>
      {settingsError ? <p role="alert" className="error">{settingsError}</p> : null}
      {snapshot?.warning ? <p role="status" className="muted">{snapshot.warning}</p> : null}
      <WatchedFolder folder={snapshot?.settings.watchedFolder ?? null} onChoose={snapshot ? chooseFolder : undefined} pending={choosing} />
      <WatcherControls />
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
