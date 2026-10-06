# Screenshot Folder Watcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Watch the configured screenshot directory and emit each newly created supported image as a deduplicated `ScreenshotFile`.

**Architecture:** Keep filesystem-specific observation in a new `folder_watcher` Rust module. A pure event-processing boundary filters create events, validates supported image files, canonicalizes paths, and deduplicates emissions; a worker owns the `notify` watcher and stop channel. Tauri commands own application state and expose start/stop lifecycle without adding UI controls.

**Tech Stack:** Rust 2021, Tauri 2, `notify`, `std::sync::mpsc`, `tempfile`, existing `ScreenshotFile` and `is_supported_image_file` model.

**Spec:** `docs/superpowers/specs/2026-10-05-screenshot-folder-watcher-design.md`

## Global Constraints

- Watch one configured directory non-recursively.
- Accept only PNG, JPG, and JPEG files, case-insensitively, through `is_supported_image_file`.
- Emit `ScreenshotFile` values in `ProcessingState::Detected`.
- Do not emit duplicate canonical paths during one active watcher session.
- `start` validates the directory and `stop` is idempotent.
- Watcher errors must not panic or terminate the application.
- Increment application version from `0.1.3` to `0.1.4`.

## Review Focus

- A supported file can generate multiple create notifications; test that only one `ScreenshotFile` is emitted.
- A file may disappear between notification and inspection; test that the worker stays alive and later files still emit.
- A path with uppercase or mixed-case extension must pass the existing supported-image boundary; test this through event processing.
- Starting twice or stopping twice must not leak a worker or panic; test lifecycle behavior explicitly.
- A watcher error event must be contained and reported without killing the process; test that subsequent events are still handled.

### Task 1: Add watcher dependency and version metadata

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Produces the `notify` dependency used by Task 2 and version `0.1.4` consumed by packaging/build tooling.

- [ ] **Step 1: Add the `notify` dependency and update the application versions**

Set `notify = "8"` under Rust dependencies and change all three application version fields from `0.1.3` to `0.1.4`. Refresh the lockfile through Cargo without changing unrelated dependency versions.

- [ ] **Step 2: Verify dependency resolution and metadata**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: exit 0, `notify` resolves, and no unrelated source files change.

- [ ] **Step 3: Commit the configuration changes**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock package.json src-tauri/tauri.conf.json
git commit -m "build: prepare screenshot folder watcher"
```

### Task 2: Implement the deterministic event processor

**Files:**
- Create: `src-tauri/src/folder_watcher.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `ScreenshotFile::new`, `is_supported_image_file`, and `notify::Event`.
- Produces: `FolderWatcherProcessor::new()`, `FolderWatcherProcessor::process_event(&mut self, event: &notify::Event) -> Result<Vec<ScreenshotFile>, FolderWatcherError>`, and `FolderWatcherError` for actionable watcher errors.

- [ ] **Step 1: Write failing unit tests for event filtering and deduplication**

Add tests in `folder_watcher.rs` for:

- `emits_one_screenshot_file_for_a_supported_create_event`, asserting the emitted original path and `Detected` state;
- `ignores_unsupported_files_directories_and_non_create_events`;
- `deduplicates_repeated_create_events_by_canonical_path`;
- `continues_after_a_file_disappears_before_inspection`;
- `accepts_mixed_case_supported_extensions`; and
- `returns_watcher_errors_without_poisoning_the_processor`.

Use temporary directories and real files. Construct `notify::Event` values directly so these tests do not depend on OS timing.

- [ ] **Step 2: Run the focused tests and verify the expected failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml folder_watcher`

Expected: FAIL because `folder_watcher` and `FolderWatcherProcessor` do not yet exist.

- [ ] **Step 3: Implement the minimal processor**

Define `FolderWatcherError` with a watcher-event variant carrying a safe message. The processor should inspect only `Create` events, ignore directories and unsupported paths, canonicalize existing files for deduplication, create `ScreenshotFile` with filesystem creation time when available, and return an empty vector for non-actionable events. Missing files are ignored; watcher errors are returned while preserving processor state.

- [ ] **Step 4: Run the focused tests and verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml folder_watcher`

Expected: all focused watcher tests PASS.

- [ ] **Step 5: Register the module and commit the processor**

Export `pub mod folder_watcher;` from `src-tauri/src/lib.rs`, then run `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and commit:

```bash
git add src-tauri/src/folder_watcher.rs src-tauri/src/lib.rs
git commit -m "feat: add screenshot watcher event processor"
```

### Task 3: Add the worker lifecycle and Tauri commands

**Files:**
- Modify: `src-tauri/src/folder_watcher.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `FolderWatcherProcessor` from Task 2 and `notify::recommended_watcher`.
- Produces: `FolderWatcher::start(directory: PathBuf, on_file: impl Fn(ScreenshotFile) + Send + Sync + 'static) -> Result<Self, FolderWatcherError>`, `FolderWatcher::stop(&mut self) -> Result<(), FolderWatcherError>`, `WatcherState`, `start_watching`, and `stop_watching` Tauri commands.

- [ ] **Step 1: Write failing lifecycle tests**

Add tests for `FolderWatcher::start` rejecting a missing path and a file path, `stop` being idempotent, and a temporary supported image created after startup reaching the callback exactly once. Add a test that an injected watcher error does not prevent a later supported create event from being processed.

- [ ] **Step 2: Run the lifecycle tests and verify the expected failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml folder_watcher::tests`

Expected: FAIL because `FolderWatcher`, `WatcherState`, and the commands do not yet exist.

- [ ] **Step 3: Implement the worker and lifecycle**

Validate the directory before spawning a thread. Use a bounded channel or equivalent stop signal, keep the `notify` watcher alive inside the worker, process events through `FolderWatcherProcessor`, invoke the callback for each emitted file, and join the worker during stop. Treat repeated stop as success and retain errors as typed values rather than panicking.

- [ ] **Step 4: Add Tauri state and commands**

Store `Option<FolderWatcher>` behind a mutex in `WatcherState`. `start_watching` stops an existing watcher before replacing it, accepts a directory string, and maps typed errors to user-safe command errors. `stop_watching` takes the watcher out of state, stops it when present, and succeeds when already stopped. Register both commands in `tauri::generate_handler!`.

- [ ] **Step 5: Run focused, Rust, frontend, and build verification**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run build
```

Expected: all commands exit 0 with no test failures or build errors.

- [ ] **Step 6: Commit the worker and command boundary**

```bash
git add src-tauri/src/folder_watcher.rs src-tauri/src/lib.rs
git commit -m "feat: watch screenshot folder lifecycle"
```

### Task 4: Final branch verification

**Files:**
- Modify: none unless verification exposes an issue.

- [ ] **Step 1: Review the complete diff against `origin/main`**

Run: `git diff --check origin/main...HEAD` and inspect the changed files for scope, version consistency, and accidental UI or unrelated dependency changes.

- [ ] **Step 2: Run the complete verification suite**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run build
```

Expected: every command exits 0.

- [ ] **Step 3: Commit any required verification-only fixes**

If verification found an implementation issue, add only the targeted fix and commit it with a focused message. Otherwise, leave the branch clean.
