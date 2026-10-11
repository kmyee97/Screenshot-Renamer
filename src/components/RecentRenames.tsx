export function RecentRenames() {
  return <section className="panel" aria-labelledby="history-heading">
    <div className="section-header"><h2 id="history-heading">Recent renames</h2><button disabled>Undo Last</button></div>
    <div className="empty-state"><p>No screenshots renamed yet.</p><p className="muted">Your recent filename changes will appear here.</p></div>
  </section>;
}
