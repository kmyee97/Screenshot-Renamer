# Epic 5 execution record

Source: Screenshot Renamer Notion backlog, four In Progress stories and their implementation plans, fetched 2026-10-11.

Order: vision contract → AI settings/credentials → OpenAI adapter → processing modes.
Each story receives its own branch, patch version, tests and merge to main.

Design: provider-independent bounded requests/results; explicit saved cloud consent; OS credential storage; a fixed HTTPS OpenAI Responses endpoint; Local never reads an image for upload; Hybrid escalates only when local signals cannot produce a useful name; AI failures retain the source and support retry. All successes use the existing naming/rename/history pipeline.

Pre-flight: settings produce mode/consent consumed by adapter/routing; the contract produces validated metadata consumed by naming. Existing naming already supports visual topic/detail. No naming API change is needed.

Ruling: use the implementation plans already supplied in the selected Notion stories as the authorized design and plan, rather than introduce new approval stages.

Baseline: Rust suite passed. Frontend dependency installation required before baseline execution.

Task 1 merged: PR #25, version 0.1.25; Notion status Done.
Task 2 review regression: distinguish a missing credential from storage failure; existence probes use native ERROR_NOT_FOUND, and removal remains available for unreadable credentials. Isolated Credential Manager entry verifies save/load/delete.
Release tooling: application-only version updater avoids matching dependency versions. No dependency version change is intended.

Task 1 verified: four contract tests passed after RED/GREEN; full Rust baseline and frontend 18 tests/build passed outside filesystem isolation. Review regression preserves window title. Version 0.1.25.

Task 2 verified: full Rust suite passed with -j 1; frontend 20 tests and production build passed. Native isolated Credential Manager lifecycle and missing-entry regression passed. Version 0.1.26.

Task 3 code verified: full native Rust suite passed with -j 1, including mocked HTTP scenarios, loopback transport bounds/redirect tests, stalled-body timeout regression, and consent-revocation retry test. Opt-in live fixture check remains pending test credential. Version 0.1.27. Windows native TLS uses the OS certificate store.
