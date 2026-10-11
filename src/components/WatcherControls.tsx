import type { SettingsSnapshot } from "../types";
type Props = { snapshot: SettingsSnapshot | null; pending: boolean; onToggle: () => void };
export function WatcherControls({ snapshot, pending, onToggle }: Props) {
  const status = snapshot?.watcherStatus ?? "Paused";
  return <section className="panel" aria-label="Automatic renaming" aria-busy={pending}>
    <div className="section-header"><span className={`status status-${status.toLowerCase()}`} role="status">{status}</span>
      <label className="toggle"><input type="checkbox" checked={snapshot?.settings.autoRename ?? false} disabled={!snapshot || pending || (!snapshot.settings.watchedFolder && !snapshot.settings.autoRename)} onChange={onToggle} /> Auto Rename</label>
    </div>
    {snapshot?.watcherError ? <p className="error" role="alert">{snapshot.watcherError}</p> : null}
    <p className="muted">Screenshots created while paused are not processed later.</p>
    <p className="muted">Local naming currently normalizes filenames. Content-based naming follows in the OCR &amp; Naming epic.</p>
  </section>;
}
