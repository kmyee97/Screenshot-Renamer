# Windows release

The release is a per-user x64 NSIS installer. It installs under the current user's Local AppData and registers normal Start menu/Add or Remove Programs entries. Node and Rust are build tools, not runtime requirements. The installer embeds the WebView2 bootstrapper; a machine missing WebView2 needs network access to download the runtime. The app otherwise bundles its frontend and native plugins.

## Build from a clean checkout

Use supported 64-bit Windows with PowerShell 7, Node.js 22.12 or newer, stable Rust with the `x86_64-pc-windows-msvc` target, and Visual Studio C++ Build Tools/Windows SDK. The Tauri CLI downloads its NSIS tooling and WebView2 bootstrapper when needed. Install the locked frontend dependencies with `npm ci`.

Run `pwsh -File scripts/build-windows.ps1`. The script rejects uncommitted changes, checks synchronized versions, runs frontend/Rust tests, builds the installer, and copies it with a SHA-256/revision manifest into `release-artifacts/`. `-SkipTests` is for packaging after the same committed revision has already passed both suites. The application metadata, generated icon sizes, and NSIS target live in `src-tauri/tauri.conf.json`; regenerate icons with `npm run tauri icon -- src-tauri/icons/app-icon.svg`.

This build is unsigned. Record signature status in the release evidence; do not treat a build as release acceptance.

## Installation, upgrades, and removal

Startup at sign-in remains opt-in. Registration writes only this app's identifier-named values in HKCU. Use the installer's normal maintenance/upgrade flow, or `/UPDATE` for an in-place upgrade that retains optional startup registration. Quit the running app before upgrading or uninstalling.

Uninstall removes binaries, shortcuts, app registration, and this app's HKCU Run/StartupApproved entries. An in-place `/UPDATE` preserves startup. Settings and rename history remain by default; the standard uninstaller offers an explicit app-data removal choice. API keys stored separately in Windows Credential Manager remain until removed using the app's Remove API key control. The selected screenshot folder is never part of the installation and is preserved.

## Release evidence

Record Windows edition/build and architecture, installer filename/SHA-256, app version, commit, date, install path, WebView2 availability, and signature status. Check install, launch, folder configuration, PNG/JPEG rename/byte preservation, collision, history, undo, pause/resume, tray Open/Settings/Pause/Resume/Quit, window hide/restore, startup enable/disable at actual sign-in, success/error notification preferences, restart persistence, upgrade, and uninstall with screenshots preserved. Track each result as Pass, Fail, or Blocked with evidence and defects. The clean-machine acceptance story and Epic remain open until these checks pass on a clean supported Windows environment.
