# Screenshot File Model Design

## Purpose

Create the shared backend representation of a newly detected screenshot. The
folder watcher creates this model after it detects a file, and downstream
services carry the same object through naming, renaming, and history
recording.

## Scope

`ScreenshotFile` represents the original file and its progress through the
rename pipeline. It must contain the original path, filename, extension,
creation time, and processing state.

It deliberately does not contain OCR or vision results, a suggested filename,
the renamed path, or persisted history data. Those belong to future analysis,
naming, rename-result, and history models respectively.

## Model

The model lives in `src-tauri/src/screenshot_file.rs` and is re-exported by
the backend library.

```rust
pub struct ScreenshotFile {
    original_path: PathBuf,
    filename: OsString,
    extension: Option<OsString>,
    created_at: Option<SystemTime>,
    state: ProcessingState,
}
```

`PathBuf` and `OsString` preserve Windows filesystem values exactly. The
optional timestamp reflects that filesystem creation time is not available on
every supported platform or filesystem. UI-facing code can convert values to
display strings at its boundary.

The model is created with a validated original path and a creation time
supplied by the watcher after its file-readiness check. The constructor derives
the filename and extension from the original path and rejects paths that do not
identify a file.

## Processing State

```rust
pub enum ProcessingState {
    Detected,
    Ready,
    Analyzing,
    Naming,
    Renaming,
    Renamed,
    Failed { stage: ProcessingStage, message: String },
}
```

`ProcessingStage` identifies the failed pipeline boundary: readiness,
analysis, naming, or rename. A state-transition method updates the state as a
service completes its portion of the pipeline. The model does not enforce a
specific service implementation or provider.

## Service Boundaries

The watcher creates `ScreenshotFile` in `Detected` state and advances it to
`Ready` after the readiness check. Analysis, naming, and rename services each
receive and return the same model while updating its state. The history service
receives the original `ScreenshotFile` alongside a future rename result, so it
can persist the original path and creation details without making the core file
model a history record.

## Error Handling

Creating a model from a path with no filename returns a domain error instead of
silently inventing a name. Failures after creation are represented by
`ProcessingState::Failed`, retaining both the pipeline stage and a user-safe
message for future UI and history services.

## Verification

Rust unit tests will verify:

- normal, extensionless, and multi-dot filenames;
- preservation of original path and creation time;
- rejection of paths without a filename; and
- state transitions, including failed states with their stage and message.

These tests establish the model boundary needed by watcher, naming, rename,
and history services without implementing those services early.
