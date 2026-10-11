# Desktop UI execution record

Source: the six implementation plans in Screenshot Renamer — Backlog; design approved October 10, 2026.

Implement in dependency order, with one feature branch, patch version bump, verified commit and merge per story:
1. Main application shell: typed section components, keyboard focus, settings entry, responsive 800×600 layout.
2. Persist application settings: versioned JSON in app-config, atomic saves, validated defaults/warnings, get/update commands.
3. Screenshot folder picker: native dialog, validate/start before committing, rollback to previous folder on failure.
4. Watcher controls: persisted Auto Rename, Watching/Paused/Error state and events; no paused backlog processing.
5. Recent renames: bounded successful history, newest first, listener before initial snapshot, clean up and deduplicate.
6. Undo controls: current filesystem eligibility, Undo Last and per-row controls, pending/error states and refresh.

Verification: failing behavior tests before implementation, frontend suite/build and Rust suite; desktop build and UI inspection. Review each diff before merging. Mark stories Done only after merge; close Epic only when every linked story is Done.

Ruling: use the existing detailed Notion plans and approved product design directly. Settings precedes folder/watcher features because both need transactional persistence. Execute inline in the existing isolated worktree.
