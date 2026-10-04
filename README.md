# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Testing

Run frontend unit and component tests once with `npm test`, or keep Vitest running while you work with `npm run test:watch`.

Write frontend tests around user-visible behavior with React Testing Library. Unit tests run in jsdom and should mock only Tauri process boundaries, such as `@tauri-apps/api/core`, rather than application UI behavior.

Run backend tests from `src-tauri` with `cargo test`. Filesystem tests must create fixtures in a `tempfile::tempdir()` directory; do not read from or write to real screenshot folders or user directories.
