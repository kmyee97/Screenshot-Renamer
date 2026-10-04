# Testing Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a runnable frontend and Rust testing foundation to Screenshot-Renamer.

**Architecture:** Vitest will extend the existing Vite configuration with jsdom and a shared React Testing Library setup module. Cargo remains the Rust test runner; `tempfile` provides isolated filesystem fixtures for backend tests.

**Tech Stack:** Vitest, jsdom, React Testing Library, `@testing-library/jest-dom`, Rust `cargo test`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-10-03-testing-setup-design.md`

## Global Constraints

- Install no Playwright packages; end-to-end coverage is deferred until UI flows exist.
- Browser tests must run in jsdom and assert observable UI behavior.
- Rust filesystem tests must use a unique `tempfile::tempdir()` fixture; never access a user directory or real screenshot folder.
- Run frontend tests from the repository root and Rust tests from `src-tauri`.
- Report separately if Rust verification cannot run because the shell lacks a Rust toolchain.

## Review Focus

- Repeated frontend tests must not leak rendered DOM between cases; Task 1 tests shared cleanup.
- A test command must run once and exit successfully in CI; Task 1 verifies `npm test` rather than watch mode.
- The app must render without starting a Tauri process; Task 1 mocks only the external `invoke` boundary.
- Rust fixture writes must stay within a unique temporary directory; Task 2 writes and reads a fixture through `tempdir()`.
- Missing Rust tooling must be reported rather than disguised as a passing backend suite; Task 2 records the actual `cargo test` result.

---

### Task 1: Frontend test runner and smoke coverage

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `vite.config.ts`
- Create: `src/test/setup.ts`
- Create: `src/App.test.tsx`
- Modify: `README.md`

**Interfaces:**
- Consumes: `App` default export from `src/App.tsx`.
- Produces: `npm test` for one-shot Vitest execution and `npm run test:watch` for interactive development.

- [ ] **Step 1: Install the frontend test dependencies**

Run: `npm install --save-dev vitest jsdom @testing-library/react @testing-library/dom @testing-library/jest-dom`

Result: `package.json` and `package-lock.json` record the compatible dev dependencies.

- [ ] **Step 2: Configure Vitest in `vite.config.ts`**

Add a Vitest test block with `environment: "jsdom"`, `setupFiles: ["./src/test/setup.ts"]`, and `globals: true`. Preserve the existing React and Tauri development-server configuration.

- [ ] **Step 3: Add scripts and shared browser-test setup**

Add `test: "vitest run"` and `test:watch: "vitest"` to `package.json`. In `src/test/setup.ts`, import `@testing-library/jest-dom/vitest` and register `cleanup` with Vitest's `afterEach`.

- [ ] **Step 4: Add the app smoke test**

Create `src/App.test.tsx`. Mock only `@tauri-apps/api/core`'s external `invoke` function, render `<App />`, and assert the visible main heading and enabled `Greet` submit button. The test protects against a broken React/Vitest/jsdom setup or an accidental loss of the primary UI.

- [ ] **Step 5: Verify frontend behavior and build output**

Run: `npm test` and `npm run build`

Expected: Vitest completes without failures and TypeScript/Vite produce a production build.

- [ ] **Step 6: Document frontend test commands**

Add the root test and watch commands to `README.md`, including the principle that tests should target user-visible behavior and mock only Tauri process boundaries.

- [ ] **Step 7: Commit the frontend test foundation**

```bash
git add package.json package-lock.json vite.config.ts src/test/setup.ts src/App.test.tsx README.md
git commit -m "test: add Vitest and React Testing Library"
```

### Task 2: Rust runner coverage and filesystem fixture convention

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `README.md`

**Interfaces:**
- Consumes: the private `greet(name: &str) -> String` command function.
- Produces: `cargo test` unit-test coverage and a `tempfile` dev dependency available to future backend integration tests.

- [ ] **Step 1: Add test-only Rust dependencies**

Add a `[dev-dependencies]` section in `src-tauri/Cargo.toml` containing `tempfile = "3"`.

- [ ] **Step 2: Add unit and temporary-filesystem tests**

In the existing `#[cfg(test)]` module in `src-tauri/src/lib.rs`, add:

```rust
#[test]
fn greet_formats_the_supplied_name() {
    assert_eq!(greet("Ari"), "Hello, Ari! You've been greeted from Rust!");
}
```

Add a second test that creates `tempfile::tempdir()`, writes `fixture.txt` beneath `temp_dir.path()`, and reads back the exact fixture content. This demonstrates the required isolated-fixture pattern without touching user files.

- [ ] **Step 3: Verify Rust tests**

Run: `cargo test` from `src-tauri`

Expected: Cargo builds the dev dependency and both tests pass. If the command is unavailable, record the `command not found` result as a verification limitation.

- [ ] **Step 4: Document Rust test and fixture conventions**

Add the `cd src-tauri; cargo test` command and the `tempfile::tempdir()` convention to `README.md`.

- [ ] **Step 5: Commit the Rust test foundation**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs README.md
git commit -m "test: add Rust test foundation"
```
