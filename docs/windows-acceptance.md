# Windows V1 acceptance record

Decision: **Blocked; V1 is not accepted.** No clean Windows Sandbox or VM is available on the current host. This branch prepares the acceptance story and reserves version 0.1.34; it must remain a draft until its final installer passes the cases below. Main contains installer implementation version 0.1.33.

## Available evidence

The source artifact is `release-artifacts/Screenshot Renamer_0.1.33_x64-setup.exe`, built from committed revision `7b982ff98228f20c22b50f4f166e8616d4efeb60`. SHA-256: `C206303FC4EFBFF4558534304596EEAD99CC08A23944214E20360FE7FBB3DE93`. The accompanying `.release.json` records its revision, timestamp, architecture, per-user installation and unsigned status. These local artifacts are ignored by Git and must be copied to the test environment separately.

The development host is Windows 11 Pro 10.0.26300, x64. WindowsSandbox.exe, Hyper-V Get-VM, VirtualBox VBoxManage and VMware vmrun were not available. A clean Windows environment with no development checkout, Node or Rust is required to close both release stories. Installing on this existing host does not establish that requirement.

Automated verification passed for the installer implementation: 142 Rust tests, 28 frontend tests, production frontend build, rustfmt and version synchronization. Native QA verified hidden monitoring, screenshot byte preservation, simulated login launch, single-instance restoration and opt-in startup registration. The packaged host smoke test used a separate product/identifier, `com.kahlil.yee.screenshotrenamer.packagingqa`, rather than the release identity:

| Case | Host QA result | Limit |
| --- | --- | --- |
| Per-user installation | Pass; installer exit 0, executable and uninstaller present | Existing development host; QA identity |
| Launch and bundled frontend | Pass; native window displayed app, history database created | WebView2 already available |
| App metadata and registration | Pass; name/version/publisher, Start menu and HKCU uninstall entry present | QA product name |
| Startup default | Pass; no QA Run registration after installation | Actual sign-in not tested |
| Same-version `/UPDATE` | Pass; exit 0; seeded QA startup/settings retained | Cross-version upgrade not tested |
| Uninstall | Pass; exit 0; binary, shortcut, uninstall entry and QA Run/StartupApproved values removed | App closed before uninstall |
| Data retention | Pass; settings/history retained and screenshot SHA-256 unchanged | Disposable QA data |

The preserved PNG's SHA-256 was `3832FE1C2F9B24747F358EFFA305BC3513BF71637E1D09D807CA9755E754D0FA`. First launch displayed the existing missing-settings/defaults notice. The monitoring panel still has outdated copy saying content-based naming is forthcoming. Record their UX impact during clean acceptance; neither observation establishes a failed rename. Packaged notifications and tray menu interactions remain unverified.

## Prepare the clean environment

1. Record Windows edition/build, architecture, tester, date, WebView2 availability and any unsigned-installer prompt. Use a normal user account. Keep a VM snapshot or a clean baseline for reruns.
2. Produce the final candidate from a clean committed acceptance branch using `pwsh -File scripts/build-windows.ps1`. Record the new artifact version, revision and SHA-256; the 0.1.33 checksum above does not apply to a 0.1.34 rebuild. Transfer only the installer, manifest, this checklist and disposable fixtures to the VM.
3. Create a screenshot folder outside the install directory. Copy `src-tauri/tests/fixtures/ocr-text.png`, `ocr-text.jpg` and `ocr-blank.png`; add a non-image file. Preserve original copies outside the watched folder and record their hashes. Also take a real Windows screenshot during the test.
4. Copy [windows-acceptance.csv](windows-acceptance.csv) as the evidence sheet. Replace Blocked with Pass or Fail only after observing the result. Record actual names, hashes, screenshots/log references and defect links. Do not enter API keys or unrelated user data into the evidence.

## Run and record

Run every case in the CSV. For collisions, first obtain a deterministic generated name, then place a different file at that name and process another equivalent fixture; verify the suffix and both hashes. For undo collisions, occupy the original filename with different bytes before Undo and confirm neither file changes. Confirm failed operations remain visible and recoverable.

Test real sign-in only in the clean VM: enable Start with Windows, sign out/in, confirm hidden monitoring, open the app, disable startup and repeat sign-in. Use an older installer built with the same release identity for the cross-version `/UPDATE` case. Quit before upgrade/uninstall. Check startup entries and retained app data after removal, and verify every screenshot hash again. Test missing-WebView2 bootstrap separately if the baseline permits it.

Do not upload screenshots for AI testing unless the tester chooses cloud processing and supplies their own consent/key. Local mode covers the required screenshot journey.

## Completion gate

All required cases must pass on the final candidate. Resolve failures and rebuild/retest affected cases; keep the installer version, checksum and revision paired with the evidence. Attach the completed sheet and defects to the Notion acceptance story, record a release decision, merge the acceptance branch, and mark the installer and acceptance stories Done. Re-query all stories under Windows & Release before marking Epic 6 Done.
