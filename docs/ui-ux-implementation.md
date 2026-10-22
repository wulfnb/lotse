# UI/UX implementation — 22 October 2026

All 16 findings from [the review](ui-ux-review.md) have implementation changes.
The original review describes the pre-change experience. This document records
the resulting behavior and the limits of verification; it does not certify every
OS, assistive technology or device configuration.

## Changes by review item

| Item | Implemented behavior | Main sources |
| --- | --- | --- |
| 1. Website heading | Readable solid text; unsupported gradient clipping is no longer required. | `site/index.html` |
| 2. File selection | Desktop Browse opens a native file dialog, with accepted extension filters and multiple-selection support. Path entry remains available; invalid input stays open with an explanation. Docker has an authenticated local-file upload and explicit remote-path handoff. | `src/ui.rs`, `src/ux.rs`, `docker/auth-service.py`, `docker/viewer/` |
| 3. Prompt defaults | Each new prompt initializes its editable value once; accepting unchanged, clearing deliberately and cancelling remain distinct. | `src/ui.rs` |
| 4. Keyboard dialogs | Initial focus, Enter submission for authentication/prompts/file entry, Escape denial/cancellation, selection/color restoration on cancellation and focus restoration. Modal input no longer triggers browser shortcuts behind the dialog. | `src/ui.rs`, `src/ux.rs`, `src/capabilities_ui.rs`, `src/browser.rs` |
| 5. Accessibility | Platform AccessKit adapters for browser and detached tools; labeled custom controls, selected tabs, visible focus, keyboard-reachable bookmark removal and F6 address/page focus switching. Servo subtrees are grafted after the parent tree, with action routing and version-conversion checks. | `src/accessibility.rs`, `src/engine.rs`, `src/main.rs`, `src/tool_window.rs`, `src/ui.rs` |
| 6. Internal themes | Browser-owned pages use active palette variables, including form controls. Switching themes updates existing internal documents without discarding their form input. | `src/capabilities.rs`, `src/capabilities_ui.rs`, `src/browser.rs` |
| 7. Feedback | Typed success/info/warning/error notices, timed transient messages, keyboard-operable Details and Copy, and Show in folder for saved screenshots. Persistent errors remain dismissible. | `src/ux.rs`, `src/ui.rs`, `src/engine.rs` |
| 8. Compact layouts | Below 1100 logical pixels, sidebar tabs become a top strip, the library becomes a constrained overlay and side-docked tools display at the bottom. Saved layout preferences are preserved. | `src/ui.rs`, `src/ux.rs`, `src/devtools_ui.rs` |
| 9. Network footer | The request table reserves measured summary height plus scrollbar space instead of a fixed one-line allowance. | `src/devtools_ui.rs` |
| 10. Downloads | Desktop Open file and Show in folder actions; Enter submits the URL; clearer empty state; protocol and connection details moved under Advanced. Docker retains the explicit save-to-local viewer flow. | `src/downloads_ui.rs`, `src/ux.rs` |
| 11. Settings | Appearance, Browsing, Privacy, Downloads and Advanced categories. Rendering diagnostics have their own developer window and toolbox entry. Internal settings navigation opens the settings surface without exposing a page-to-native mutation bridge. | `src/capabilities_ui.rs`, `src/engine.rs`, `src/devtools_ui.rs` |
| 12. Site-data deletion | Confirmation names the site and explains sign-out/offline-data consequences; cancellation is the initial focus. | `src/capabilities_ui.rs` |
| 13. Mobile website | Labeled compact menu, full-width download actions, grid columns that fit their container, visible keyboard focus and sticky-header anchor offsets. | `site/index.html`, `site/site.js` |
| 14. Docker viewport | Fit, Actual size/pan and explicitly requested remote resize modes. Full screen remains available; resize explains its fallback if the server does not support it. | `docker/viewer/viewer.js`, `docker/viewer/viewer.css` |
| 15. Copy feedback | Success is announced only after the Clipboard promise resolves. Missing API/denial selects the visible command and explains manual copying. | `site/site.js`, `tests/site_tests.mjs` |
| 16. Onboarding/titles | Theme-aware new-tab search form, saved-site links and recent-page access; measured one-line tab-title elision and fallback document icons. | `src/capabilities_ui.rs`, `src/ui.rs` |

The previously implemented release links and Docker image/password corrections
remain included in the working tree and README.

## Security and dependency details

Native file dialogs use `rfd`; the native accessibility bridge enables the
existing egui-winit AccessKit adapter. Servo and egui currently depend on adjacent
AccessKit schema versions, so the bridge converts supported serialized structures
and has regression checks for node/action preservation and real engine updates.
It never casts enum layouts or node IDs between versions.

Docker uploads require an authenticated session, the expected HTTPS Origin and a
CSRF token. Nginx also performs its normal session check before proxying uploads.
Files are limited to 32 MiB each and the upload area to 256 MiB / 100 files.
Filenames cannot contain paths. Publication uses private atomic files in random
directories under `/config/auth/uploads`; the directory is not served by nginx.
Uploads remain in the persistent profile until removed by its administrator.
Opening or revealing a downloaded file requires a separate user click.

## Initial UI/UX verification

The measurements below record the initial implementation checkpoint. See
[coverage progress](coverage-progress.md) for the expanded regression suite and
newer line measurements.

- 71 Rust unit tests: 52 library and 19 desktop, including accessibility schema
  conversion, accessible toolbar semantics, file-path validation and notice rules.
- All seven native suites passed under instrumentation: smoke, all six themes,
  capabilities, toolbox, downloads, WebDriver and media.
- Added native cases for untouched prompt defaults, intentionally empty prompt
  input, Escape cancellation, Paper/Emerald live internal-page colors, new-tab
  search/control alignment and a 720×480 library overlay that preserves page width.
- The toolbox run checks conversion of actual Servo accessibility updates, in
  addition to its existing docking, debugger, inspector and Network checks.
- 31 Python tests passed. Gateway coverage: **293/293 lines and 106/106 branches**.
- JavaScript coverage, including the new website script: **328/328 lines**.
- Disposable Docker HTTPS test passed: nginx syntax, anonymous denial, CSRF,
  Origin and size guards, current viewer assets, and private uploaded-file access
  from the browser UID. The container was removed after the test. Locally this
  used an existing Linux image for system dependencies and mounted current
  gateway/viewer/nginx files; it was not a new Linux Rust build.
- The Docker check is wired into CI using the image built by that CI run.
- Clippy with warnings denied passed for production and test-harness targets;
  normal production build, formatting and workflow lint checks passed.
- Initial desktop line measurement: **8363/11469 (72.92%)**. Core line measurement:
  **1247/1304 (95.63%)**. These are the real measurements; whole-project 100%
  line coverage is still outstanding. Existing thresholds were not reduced.

Reports and screenshots are under `target/ui-ux-implementation/` and
`target/coverage/`. The baseline check passed against the freshly measured core
and gateway reports. The whole-project completeness inventory remains incomplete
because of coverage gaps and unmeasured required OS configurations.

## Manual validation still required

Implementation and automated regression checks are complete for this change.
Before claiming cross-platform accessibility or device certification, run:

- VoiceOver and NVDA tasks through chrome, dialogs, tabs and page controls.
- Native file chooser/open/reveal behavior on each supported OS, especially Windows
  Explorer and Linux desktop portal availability.
- Mobile Safari/Chrome and touch/keyboard operation of the Docker viewport and
  local-upload-to-remote-file-chooser handoff.
- Glass themes with real desktop backgrounds, reduced transparency and increased
  contrast settings. GPU exports alone do not show the complete OS compositor.
- Fresh Windows/Linux desktop builds and first-install behavior. CI will build
  those configurations when these changes are pushed.

No commits or pushes were made for this implementation.
