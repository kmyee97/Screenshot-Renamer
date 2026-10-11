import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { errorMessage } from "../types";
type Failure = { id: string; sourcePath: string; message: string; retryable: boolean };
export function AnalysisFailures() {
  const [failures, setFailures] = useState<Failure[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const busy = useRef(false);
  useEffect(() => {
    let active = true;
    let revision = 0;
    let release: UnlistenFn | undefined;
    void (async () => {
      try {
        const unlisten = await listen<Failure[]>("analysis-failures-changed", event => {
          revision++;
          if (active) setFailures(event.payload);
        });
        if (!active) { unlisten(); return; }
        release = unlisten;
        const started = revision;
        const result = await invoke<Failure[]>("list_analysis_failures");
        if (active && revision === started) setFailures(result);
      } catch (err) { if (active) setError(errorMessage(err)); }
    })();
    return () => { active = false; release?.(); };
  }, []);
  async function retry(id: string) {
    if (busy.current) return;
    busy.current = true; setPending(id); setError(null);
    try {
      await invoke("retry_analysis", { id });
      setFailures(current => current.filter(failure => failure.id !== id));
    } catch (err) { setError(errorMessage(err)); }
    finally { busy.current = false; setPending(null); }
  }
  if (!failures.length && !error) return null;
  return <section className="panel" aria-label="Analysis failures">
    <h2>Analysis needs attention</h2>
    {error ? <p role="alert" className="error">{error}</p> : null}
    {failures.map(failure => <div key={failure.id}>
      <p>{failure.sourcePath}</p><p>{failure.message}</p>
      <button disabled={!failure.retryable || pending !== null} onClick={() => void retry(failure.id)}>{pending === failure.id ? "Retrying…" : "Retry analysis"}</button>
      {!failure.retryable ? <p className="muted">The original file is unavailable or changed. Retry is unavailable.</p> : null}
    </div>)}
    <p className="muted">Retry uses your current saved analysis settings. Select Local in Settings to retry on this device.</p>
  </section>;
}
