export type ApplicationSettings = { version: number; watchedFolder: string | null; autoRename: boolean };
export type SettingsSnapshot = { settings: ApplicationSettings; warning: string | null };
export function errorMessage(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : "The operation could not be completed.";
}
