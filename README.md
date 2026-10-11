# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Testing

Run frontend unit and component tests once with `npm test`, or keep Vitest running while you work with `npm run test:watch`.

Write frontend tests around user-visible behavior with React Testing Library. Unit tests run in jsdom and should mock only Tauri process boundaries, such as `@tauri-apps/api/core`, rather than application UI behavior.

Run backend tests from `src-tauri` with `cargo test`. Filesystem tests must create fixtures in a `tempfile::tempdir()` directory; do not read from or write to real screenshot folders or user directories.

## Rename recovery API

`rename_and_record` returns a saved history entry on success or a structured error with `id`, `category`, `sourcePath`, `attemptedDestination`, `actualPath`, `stage`, `message`, `retryable`, and `filesystemSucceeded`. Categories include `invalidName`, `collisionExhausted`, `sourceMissing`, `permissionIo`, and `historyWrite`.

Use `list_rename_failures` to restore the current failure list after attaching UI listeners. Call `retry_rename` with the failure `id` and an optional corrected `candidateStem`. Retries inspect the current file and select an available destination without overwriting. If the file already moved, retry only saves the retained history outcome after checking the destination's SHA-256 fingerprint. A recreated original, a changed destination, and repeated successful retries cannot produce another rename or duplicate success history.

The backend emits `screenshot-renamed` (history entry), `screenshot-rename-failed` (structured failure), and `screenshot-rename-retry-result` (`id`, `history`, `failure`; exactly one outcome is non-null). Rename, retry, and undo share a filesystem gate. Failed attempts remain available for the current app session; saved rename history persists across restarts. When closing the app before recovery, use the reported `actualPath` to locate the screenshot. Desktop UI controls are tracked separately in the Desktop UI Epic.
