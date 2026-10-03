# Testing Setup Design

## Purpose

Establish a fast, repeatable testing foundation for the Tauri + React screenshot-renamer. It must support frontend component behavior, Rust backend behavior, and filesystem-focused backend tests without adding end-to-end tooling before user flows exist.

## Scope

The project will use:

- Vitest for TypeScript and React unit tests.
- React Testing Library with `@testing-library/jest-dom` for user-visible component behavior.
- Cargo's built-in test runner for Rust unit and integration tests.
- The `tempfile` Rust dev dependency for isolated filesystem test fixtures.

Playwright is intentionally out of scope for this change. It will be added when the app has end-to-end UI flows worth exercising.

## Frontend Architecture

Vitest will run through the existing Vite configuration. A test block will enable the jsdom environment, register a shared setup module, and configure automatic test cleanup. Package scripts will expose a one-shot `test` command and a watch-mode command.

The shared setup module will import DOM matchers and register Testing Library cleanup after each test. Tests will live beside source files or in a clear frontend test location and will test observable behavior rather than implementation details.

One smoke test will render the current app and assert visible UI content. It verifies that TypeScript transformation, React rendering, jsdom, and DOM matchers are connected correctly.

## Rust Architecture

Rust tests will use `cargo test` directly; no additional test runner will be introduced. Backend logic will be kept in testable functions or modules outside Tauri command wiring where practical. The current command can receive a small unit test as a runner smoke check.

Backend tests that need the filesystem will use `tempfile::tempdir()` to create a unique temporary directory, write only their own fixtures within it, and assert results by reading that directory. The directory is automatically removed when its handle is dropped. No test may target a user directory or a real screenshot folder.

## Developer Experience

The README will list the frontend and Rust test commands, explain where tests belong, and note the temporary-directory convention for filesystem behavior. The JavaScript test command will be available from the project root; Rust tests run from `src-tauri`.

## Error Handling and Boundaries

The frontend setup will keep Tauri API calls mockable at the module boundary for component tests. Tests will not start a Tauri desktop process. Rust filesystem tests will propagate or assert domain errors, while fixture setup failures use explicit test failure messages.

## Verification

After configuration, verification will run the frontend test suite, the production frontend build, and `cargo test` when a Rust toolchain is available. The current shell does not expose `cargo`, so the final report will clearly distinguish a successful frontend result from any Rust verification that requires a configured Rust PATH.
