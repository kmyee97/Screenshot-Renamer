import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage, type RenameHistory } from "../types";

export function RecentRenames() {
  const [entries, setEntries] = useState<RenameHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [subscriptionError, setSubscriptionError] = useState<string | null>(null);
  const [connectionAttempt, setConnectionAttempt] = useState(0);
  const refresh = useRef<() => Promise<void>>(async () => {});
  useEffect(() => {
    let active = true;
    let request = 0;
    let unlisten: UnlistenFn | undefined;
    async function load() {
      const current = ++request;
      setLoading(true);
      try {
        const rows = await invoke<RenameHistory[]>("list_recent_history", { limit: 50 });
        if (active && current === request) {
          const unique = new Map(rows.filter(row => row.outcome.status === "succeeded").map(row => [row.id, row]));
          setEntries([...unique.values()].sort((a, b) => b.attemptedAtMs - a.attemptedAtMs || b.id.localeCompare(a.id)));
          setError(null);
        }
      } catch (failure) { if (active && current === request) setError(errorMessage(failure)); }
      finally { if (active && current === request) setLoading(false); }
    }
    refresh.current = load;
    void (async () => {
      try {
        const release = await listen("screenshot-renamed", () => { if (active) void load(); });
        if (!active) { release(); return; }
        unlisten = release;
        setSubscriptionError(null);
      } catch (failure) { if (active) setSubscriptionError(errorMessage(failure)); }
      if (active) await load();
    })();
    return () => { active = false; unlisten?.(); };
  }, [connectionAttempt]);
  return <section className="panel" aria-labelledby="history-heading" aria-busy={loading}>
    <div className="section-header"><h2 id="history-heading">Recent renames</h2><div className="history-actions"><button onClick={() => subscriptionError ? setConnectionAttempt(value => value + 1) : void refresh.current()} disabled={loading}>Refresh history</button><button disabled>Undo Last</button></div></div>
    {subscriptionError ? <p role="alert" className="error">{subscriptionError}</p> : null}
    {error ? <p role="alert" className="error">{error}</p> : null}
    {loading ? <p role="status" className="muted">Loading history…</p> : null}
    {!loading && !error && !entries.length ? <div className="empty-state"><p>No screenshots renamed yet.</p><p className="muted">Your recent filename changes will appear here.</p></div> : null}
    {entries.length ? <ol className="rename-list">{entries.map(entry => <li key={entry.id}>
      <div className="rename-names"><span title={entry.originalPath}>{entry.originalName}</span><span aria-hidden="true">→</span><strong title={entry.newPath ?? undefined}>{entry.newName}</strong></div>
      <div className="rename-meta"><time dateTime={new Date(entry.attemptedAtMs).toISOString()}>{new Date(entry.attemptedAtMs).toLocaleString()}</time><span>{entry.undoStatus.status === "succeeded" ? "Undone" : "Renamed"}</span></div>
      {entry.undoStatus.status === "failed" ? <p className="error">Undo failed: {entry.undoStatus.details?.reason}</p> : null}
    </li>)}</ol> : null}
  </section>;
}
