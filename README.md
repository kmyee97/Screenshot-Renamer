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

## Local OCR and filename styles

Auto Rename waits for each PNG/JPEG to become ready, extracts text with Windows OCR using an installed user language, cleans the text conservatively, and names the image from provider-independent metadata. OCR never uploads images. Missing language support, corrupt images, files over 32 MiB, and dimensions beyond Windows OCR's supported limit produce a visible warning and use remaining window/timestamp context. Window hints are captured near the file event, limited to an executable name and a short title, with a 30 ms lookup deadline. They are optional and may be unavailable or differ from the window shown in the image.

In Settings, choose Descriptive, Minimal, Coding, Work/Research, or Receipts/Orders. An optional custom template overrides the preset. Supported tokens are `{app}`, `{topic}`, `{detail}`, and `{timestamp}` (capture time in Unix milliseconds); `{{` and `}}` produce literal braces. Missing fields are skipped, repeated separators are collapsed, and empty results fall back to `screenshot-{timestamp}`. Unknown or unclosed tokens are rejected. The preview uses the same Rust engine as Auto Rename. Preferences persist on this device; older settings files receive default naming rules.

Names use lowercase terms, conservative stop words, Windows sanitization, and a default 120 UTF-16-unit stem limit. The engine reserves space for the original extension and the rename service's collision suffix; it never replaces an existing destination. Raw OCR and cleaned text remain separate transient metadata and are not saved to rename history.

Windows OCR operations share a three-second deadline and cancel stalled work before fallback naming. Watcher lifecycle commands run on a background thread. Saving naming preferences preserves in-flight analysis and queued screenshots: the current screenshot retains its starting rules, and subsequent screenshots use the saved rules.

## AI privacy preferences

New installs and older settings default to Local. In Settings, selecting AI or Hybrid requires checking the explicit screenshot-upload consent box and saving the preference. Returning to Local disables cloud analysis. The provider and model choice persist in settings; API keys are stored separately in Windows Credential Manager under `ScreenshotRenamer/OpenAI`, never in settings JSON or rename history. The UI accepts a key only for entry, clears it immediately after submission, and can remove a saved key. Saving a key alone grants no upload consent. An unavailable credential store or missing key is a recoverable configuration error.

The first vision adapter uses OpenAI Responses with configurable `gpt-4.1-mini` as the default. See [adapter limits, API costs, retention and test instructions](docs/openai-vision.md). Image uploads use a fixed HTTPS endpoint and Windows native TLS certificate validation.

Local analyzes screenshots with on-device OCR and window context and never requests cloud analysis. AI skips OCR and asks the configured vision provider for naming context. Hybrid uses local analysis first, then asks vision only when there is no useful topic or the selected filename template would produce a generic fallback. If vision fails, Hybrid keeps the valid local name and shows the failure reason. Every mode uses the same filename sanitation, collision protection, rename history and undo pipeline.

Each ready screenshot retains its starting mode and naming rules. Queued screenshots use the latest saved preferences; changing to Local or revoking consent blocks subsequent uploads and provider retries for work already in progress. AI failures preserve the source and appear under Analysis needs attention. Retry uses current saved settings and an ID retained by the backend; a replaced or changed source cannot be retried. Failure recovery is limited to the current app session. Provider requests run on the watcher worker with a 30-second total deadline, keeping UI commands off the analysis thread.

The OCR contracts include text PNG/JPEG and blank PNG fixtures. Generate these fixtures with Arial 44 pt text `SCREENSHOT ORDER 12345` on a 960×180 white canvas. Run native OCR and filesystem tests outside filesystem-isolating sandboxes, which can change Windows collision error codes. Windows native tests need an installed OCR language matching the user's profile.
