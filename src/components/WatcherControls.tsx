export function WatcherControls() {
  return <section className="panel" aria-label="Automatic renaming">
    <div className="section-header"><span className="status" role="status">Paused</span><label className="toggle"><input type="checkbox" disabled /> Auto Rename</label></div>
    <p className="muted">Screenshots created while paused are not processed later.</p>
  </section>;
}
