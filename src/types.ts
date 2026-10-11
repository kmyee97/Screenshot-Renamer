export type ApplicationSettings = { version: number; watchedFolder: string | null; autoRename: boolean };
export type WatcherEvent = { watcherStatus: "Watching" | "Paused" | "Error"; watcherError: string | null };
export type SettingsSnapshot = WatcherEvent & { settings: ApplicationSettings; warning: string | null };
export function errorMessage(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : "The operation could not be completed.";
}
export interface RenameHistory {
  id: string;
  originalPath: string;
  originalName: string;
  newPath: string | null;
  newName: string | null;
  attemptedAtMs: number;
  outcome: { status: "pending" | "succeeded" | "failed"; details?: { message: string } };
  undoStatus: { status: "notAttempted" | "succeeded" | "failed"; details?: { at_ms: number; reason?: string } };
}
