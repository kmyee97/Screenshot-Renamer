# Screenshot Folder Watcher Design

## Purpose

Monitor the configured screenshot directory and turn newly created supported
image files into `ScreenshotFile` values for the downstream processing
pipeline.

## Scope

The watcher owns filesystem observation and event deduplication. It does not
perform readiness checks, OCR, naming, renaming, history persistence, or UI
state management. It watches one configured directory non-recursively and
accepts only the supported image formats already defined by
`is_supported_image_file`: PNG, JPG, and JPEG, case-insensitively.

## Components

### `folder_watcher`

The new `src-tauri/src/folder_watcher.rs` module provides a thread-safe
watcher service with explicit `start` and `stop` operations. `start` validates
the directory, creates a filesystem watcher, and begins receiving events on a
worker thread. `stop` signals the worker, joins it, and is safe to call when
already stopped.

The service emits a newly constructed `ScreenshotFile` through a callback
owned by the caller. It records canonical paths that have already been
emitted, so filesystem backends that report the same creation more than once
do not produce duplicate pipeline inputs. The set lives for the lifetime of
the active watcher and is reset when a new watcher starts.

### Tauri boundary

The backend library exposes Tauri commands to start and stop the watcher. The
commands operate on application-managed watcher state and return actionable
errors rather than panicking. UI controls and event subscriptions are outside
this story.

## Data flow

1. The caller requests `start` with a configured directory.
2. The watcher validates that the directory exists and is a directory.
3. A create event is received for a path within that directory.
4. The watcher ignores directories, unsupported extensions, and paths already
   emitted during the active session.
5. The watcher constructs a `ScreenshotFile` with the file's creation time when
   available, otherwise `None`.
6. The callback receives the file in `ProcessingState::Detected`.
7. A stop request ends event processing and joins the worker cleanly.

Modification, removal, rename, and watcher-error events do not create
pipeline inputs. Watcher errors are reported through the service result or
callback boundary and do not terminate the process unexpectedly.

## Error handling

- Starting with a missing path or a non-directory returns a typed startup
  error.
- Failure to create or register the underlying filesystem watcher returns a
  typed startup error.
- A malformed event or a file that disappears before inspection is ignored as
  a non-actionable event; the watcher remains alive.
- Stop and worker-channel failures return typed errors where recovery is not
  possible, while repeated stop calls remain harmless.

## Versioning

The application version advances from `0.1.3` to `0.1.4` in `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. Adding the watcher
dependency also updates `src-tauri/Cargo.lock`.

## Verification

Rust tests will verify:

- a supported image creation is emitted as a `ScreenshotFile`;
- unsupported and non-file paths are ignored;
- duplicate create notifications emit only once;
- start and stop complete cleanly, including an idempotent stop;
- invalid watcher directories return an error without starting a worker; and
- the existing screenshot-file and supported-image tests remain green.

The full Rust test suite and frontend test/build commands will run before the
feature is considered complete.
