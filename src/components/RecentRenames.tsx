import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage, type RenameHistory, type HistoryItem, type RecentHistory } from "../types";

export function RecentRenames() {
  const [entries, setEntries] = useState<HistoryItem[]>([]);
  const [undoLast, setUndoLast] = useState<RenameHistory | null>(null);
  const [pendingId, setPendingId] = useState<string | null>(null);
  const [undoError, setUndoError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const pending = useRef(false);
  const mounted = useRef(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [subscriptionError, setSubscriptionError] = useState<string | null>(null);
  const [connectionAttempt, setConnectionAttempt] = useState(0);
  const refresh = useRef<() => Promise<void>>(async () => {});
  useEffect(() => {
    let active = true;
    let request = 0;
    mounted.current = true;
    const releases: UnlistenFn[] = [];
    async function load() {
      if (!active) return;
      const current = ++request;
      setLoading(true);
      try {
        const value = await invoke<RecentHistory>("get_recent_history", { limit: 50 });
        if (active && current === request) {
          const unique = new Map(value.entries.filter(row => row.outcome.status === "succeeded").map(row => [row.id, row]));
          setEntries([...unique.values()].sort((a, b) => b.attemptedAtMs - a.attemptedAtMs || b.id.localeCompare(a.id)));
          setUndoLast(value.undoLast);
          setError(null);
        }
      } catch (failure) { if (active && current === request) setError(errorMessage(failure)); }
      finally { if (active && current === request) setLoading(false); }
    }
    refresh.current = load;
    void (async () => {
      try {
        for (const event of ["screenshot-renamed", "screenshot-undo-result"]) {
          const release = await listen(event, () => { if (active) void load(); });
          if (!active) { release(); return; }
          releases.push(release);
        }
        setSubscriptionError(null);
      } catch (failure) { if (active) setSubscriptionError(errorMessage(failure)); }
      if (active) await load();
    })();
    return () => { active = false; mounted.current = false; releases.forEach(release => release()); };
  }, [connectionAttempt]);
  async function undo(id: string) {
    if (pending.current || loading) return;
    pending.current = true;
    setPendingId(id);
    setUndoError(null);
    setNotice(null);
    try {
      const restored = await invoke<RenameHistory>("undo_history", { id });
      if (mounted.current) setNotice(`Restored ${restored.originalName}.`);
    } catch (failure) { if (mounted.current) setUndoError(errorMessage(failure)); }
    finally {
      await refresh.current();
      pending.current = false;
      if (mounted.current) setPendingId(null);
    }
  }
  return <section className="panel" aria-labelledby="history-heading" aria-busy={loading}>
    <div className="section-header"><h2 id="history-heading">Recent renames</h2><div className="history-actions"><button onClick={() => subscriptionError ? setConnectionAttempt(value => value + 1) : void refresh.current()} disabled={loading || !!pendingId}>Refresh history</button><button disabled={loading || !!pendingId || !undoLast || !!error} title={undoLast ? `Undo rename of ${undoLast.originalName}` : "No eligible rename to undo"} onClick={() => undoLast && void undo(undoLast.id)}>Undo Last</button></div></div>
    {undoError ? <p role="alert" className="error">{undoError}</p> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {subscriptionError ? <p role="alert" className="error">{subscriptionError}</p> : null}
    {error ? <p role="alert" className="error">{error}</p> : null}
    {loading ? <p role="status" className="muted">Loading history…</p> : null}
    {!loading && !error && !entries.length ? <div className="empty-state"><p>No screenshots renamed yet.</p><p className="muted">Your recent filename changes will appear here.</p></div> : null}
    {entries.length ? <ol className="rename-list">{entries.map(entry => <li key={entry.id}>
      <div className="rename-names"><span title={entry.originalPath}>{entry.originalName}</span><span aria-hidden="true">→</span><strong title={entry.newPath ?? undefined}>{entry.newName}</strong></div>
      <div className="rename-meta"><time dateTime={new Date(entry.attemptedAtMs).toISOString()}>{new Date(entry.attemptedAtMs).toLocaleString()}</time><span>{entry.undoStatus.status === "succeeded" ? "Undone" : "Renamed"}</span></div>
      {entry.undoStatus.status === "failed" ? <p className="error">Undo failed: {entry.undoStatus.details?.reason}</p> : null}
      <div className="row-undo"><button aria-label={`Undo rename of ${entry.originalName}`} disabled={loading || !!pendingId || !entry.canUndo || !!error} onClick={() => void undo(entry.id)}>{pendingId === entry.id ? "Restoring…" : "Undo"}</button>{entry.undoReason ? <span className="muted">{entry.undoReason}</span> : null}</div>
    </li>)}</ol> : null}
  </section>;
}
