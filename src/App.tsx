import { useState } from "react";
import { WatchedFolder } from "./components/WatchedFolder";
import { WatcherControls } from "./components/WatcherControls";
import { RecentRenames } from "./components/RecentRenames";
import "./App.css";

function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  return (
    <main className="app-shell">
      <header className="app-header">
        <div><p className="eyebrow">YOUR SCREENSHOTS, ORGANIZED</p><h1>Screenshot Renamer</h1></div>
        <button aria-expanded={settingsOpen} aria-controls="settings-panel" onClick={() => setSettingsOpen(!settingsOpen)}>Settings</button>
      </header>
      <WatchedFolder folder={null} />
      <WatcherControls />
      <RecentRenames />
      {settingsOpen ? <section className="panel" id="settings-panel" aria-labelledby="settings-heading">
        <div className="section-header"><h2 id="settings-heading">Settings</h2><button onClick={() => setSettingsOpen(false)}>Close settings</button></div>
        <p>Folder and Auto Rename preferences will appear here when configured.</p>
      </section> : null}
    </main>
  );
}
export default App;
