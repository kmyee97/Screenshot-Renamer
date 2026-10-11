import { useEffect, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
export function PipelineNotice() {
  const [notice, setNotice] = useState<{ message: string; failed: boolean } | null>(null);
  useEffect(() => {
    let active = true;
    const releases: UnlistenFn[] = [];
    for (const [name, failed] of [["screenshot-analysis-warning", false], ["screenshot-naming-failed", true], ["screenshot-analysis-completed", false]] as const) {
      void listen<string>(name, event => {
        if (active) setNotice({ message: event.payload, failed });
      }).then(release => { if (active) releases.push(release); else release(); }).catch(() => {});
    }
    return () => { active = false; releases.forEach(release => release()); };
  }, []);
  return notice ? <p role={notice.failed ? "alert" : "status"} className={notice.failed ? "error" : "muted"}>{notice.message}</p> : null;
}
