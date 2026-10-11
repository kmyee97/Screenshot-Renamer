export type ApplicationSettings = { version: number; watchedFolder: string | null; autoRename: boolean };
export type WatcherEvent = { watcherStatus: "Watching" | "Paused" | "Error"; watcherError: string | null };
export type SettingsSnapshot = WatcherEvent & { settings: ApplicationSettings; warning: string | null };
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return "The operation could not be completed.";
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
export interface HistoryItem extends RenameHistory { canUndo: boolean; undoReason: string | null }
export interface RecentHistory { entries: HistoryItem[]; undoLast: RenameHistory | null }
