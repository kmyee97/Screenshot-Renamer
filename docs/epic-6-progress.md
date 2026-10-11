# Windows integration execution record

Source: the six existing implementation/acceptance plans in the Notion Screenshot Renamer backlog. User requested implementation, a separate feature branch and version bump for each story, merge to main, and Done status only after completion.

Order: system tray (0.1.29), minimize to tray (0.1.30), startup at login (0.1.31), notifications (0.1.32), Windows installer (0.1.33), clean-machine acceptance (0.1.34).

Ruling: execute the supplied Notion plans inline in this existing isolated worktree. Reuse the controller for tray transitions; maintain opt-in startup and cloud consent. Use errors-only notification defaults and per-user NSIS packaging. Acceptance requires actual clean Windows installation and user journey evidence; automated unit tests alone cannot satisfy it.

## Progress

- Discovery: all six remaining stories and Epic 6 are In Progress; all other Epics are Done. No local Windows Sandbox executable or Hyper-V commands detected yet. User authorized an available local Sandbox/VM if present.
- Tray implementation: 131 Rust tests and 24 frontend tests passed; production build and rustfmt passed. Independent review corrected publication races and settings registration ordering. Isolated native QA build launches. Merged PR 29; story Done. Packaged tray action validation remains in release acceptance.

- Minimize implementation: version 0.1.30; 134 Rust and 25 frontend tests passed, production build and rustfmt passed. Independent review corrected shutdown and deferred-hide races. Native QA configured a disposable folder, enabled Watching, minimized to a hidden window, and renamed a new PNG with its SHA-256 bytes unchanged. Packaged tray actions and clean-machine behavior remain in release acceptance.

- Startup implementation: version 0.1.31; 139 Rust and 27 frontend tests passed; build and rustfmt passed. Independent review corrected per-user scope, partial-registration rollback, startup error visibility, and unavailable-folder preference saves. Native QA simulated login launch hidden with active monitoring, restored one window on manual relaunch, enabled startup using the quoted QA executable in its own HKCU Run entry, then disabled and verified removal. Actual sign-out/sign-in and installer cleanup remain in clean-machine acceptance.
- Ruling: use explicit HKCU registration keyed by application identifier because the generic autostart plugin can write HKLM when elevated and removes both scopes. Desktop-only preferences persist without validating or restarting the watcher.
