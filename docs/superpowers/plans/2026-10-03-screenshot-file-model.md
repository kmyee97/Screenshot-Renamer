# Screenshot File Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide a tested Rust `ScreenshotFile` model that preserves source-file facts and tracks its passage through the screenshot rename pipeline.

**Architecture:** The model resides in a focused backend module and is re-exported from the Tauri library crate. Its constructor derives file-name data from a validated path, while accessors preserve filesystem-native values. A separate processing-state enum represents pipeline progress and failures without introducing OCR, generated-name, or history concerns.

**Tech Stack:** Rust 2021, standard library filesystem types and time types, Cargo test.

**Spec:** `docs/superpowers/specs/2026-10-03-screenshot-file-model-design.md`

## Global Constraints

- Preserve original path, filename, extension, creation time, and processing state in `ScreenshotFile`.
- Use `PathBuf` and `OsString` for filesystem-native values; do not convert to UI strings in the model.
- Creation time is `Option<SystemTime>` because it may be unavailable on a filesystem.
- Keep OCR/vision metadata, generated names, final paths, and history records outside this model.
- A path without a filename must produce a domain error.
- Do not add dependencies.

## Review Focus

- Extensionless files retain their complete filename and expose no extension; test in Task 1.
- Multi-dot filenames retain their complete filename and expose only the final extension; test in Task 1.
- A path ending in a directory or root is rejected instead of generating a blank filename; test in Task 1.
- The supplied creation time and original path remain unchanged; test in Task 1.
- Failed processing retains both its originating stage and diagnostic message; test in Task 1.

---

### Task 1: Create the screenshot-file model and lifecycle

**Files:**
- Create: `src-tauri/src/screenshot_file.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `ScreenshotFile::new(original_path: PathBuf, created_at: Option<SystemTime>) -> Result<ScreenshotFile, ScreenshotFileError>`.
- Produces: `original_path(&self) -> &Path`, `filename(&self) -> &OsStr`, `extension(&self) -> Option<&OsStr>`, and `created_at(&self) -> Option<&SystemTime>`.
- Produces: `ProcessingState`, `ProcessingStage`, `ScreenshotFile::state(&self) -> &ProcessingState`, and `ScreenshotFile::transition_to(&mut self, state: ProcessingState)`.
- Produces: public re-exports of all model types from `screenshot_renamer_lib`.

- [ ] **Step 1: Write the failing constructor and accessor tests in `src-tauri/src/screenshot_file.rs`**

Test a `C:\\Screenshots\\Screenshot_2026-10-03_141922.png` path with a fixed `SystemTime`, an extensionless `Screenshot_141922` path, a `report.final.png` path, and a directory-only path. Assert exact source-path, filename, extension, timestamp, and missing-filename error behavior. Also assert that a new model begins in `Detected`, transitions to `Ready` and `Renaming`, and retains a `Failed { stage: ProcessingStage::Rename, message: "destination already exists" }` value exactly.

- [ ] **Step 2: Run the focused test module to verify it fails**

Run: `cargo test screenshot_file --lib`

Expected: FAIL because the `screenshot_file` module and `ScreenshotFile` interface do not exist.

- [ ] **Step 3: Implement the model and lifecycle in `src-tauri/src/screenshot_file.rs`**

Define `ScreenshotFile`, `ScreenshotFileError::MissingFilename { path: PathBuf }`, `ProcessingState::{Detected, Ready, Analyzing, Naming, Renaming, Renamed, Failed { stage, message }}`, and `ProcessingStage::{Readiness, Analysis, Naming, Rename}`. Implement the constructor, accessors, state accessor, and direct transition method. Derive filename and extension with `Path::file_name` and `Path::extension`; initialize state to `Detected`.

- [ ] **Step 4: Re-export the model from `src-tauri/src/lib.rs`**

Declare the module and re-export its public model and lifecycle types so future watcher, naming, rename, and history modules can import one shared type family from the library crate.

- [ ] **Step 5: Run the focused model tests to verify they pass**

Run: `cargo test screenshot_file --lib`

Expected: PASS with all constructor and accessor cases green.

- [ ] **Step 6: Commit the complete screenshot-file model**

```bash
git add src-tauri/src/screenshot_file.rs src-tauri/src/lib.rs
git commit -m "feat: add screenshot file model"
```

### Task 2: Verify backend compatibility

**Files:**
- Verify: `src-tauri/src/lib.rs`
- Verify: `src-tauri/src/screenshot_file.rs`

**Interfaces:**
- Consumes: the public model and state APIs from Task 1.
- Produces: a backend crate that compiles with the existing Tauri application wiring unchanged.

- [ ] **Step 1: Run the complete Rust test suite**

Run: `cargo test`

Expected: PASS, including existing greeting and temporary-filesystem tests plus the new screenshot-model coverage.

- [ ] **Step 2: Run formatting validation**

Run: `cargo fmt --check`

Expected: PASS with no formatting differences.

- [ ] **Step 3: Review the final diff and working tree**

Run: `git diff HEAD~1..HEAD --check` and `git status --short --branch`

Expected: no whitespace errors and no uncommitted implementation files.
