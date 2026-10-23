# UI/UX implementation — 22 October 2026

All 16 findings from [the review](ui-ux-review.md) have implementation changes.
The [23 October follow-up](#23-october-follow-up) tracks the separate second-pass backlog.
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

## 23 October follow-up

The [second-pass proposals](ui-ux-proposals.md#implementation-status--23-october-2026)
are a separate backlog from the original 16 findings. This checkpoint implements
A1, A2, A7, B1, the B4 modal alternative, the B5 wording/row changes, and parts of
B2, B6 and C2. C4 has platform implementations with Windows/Linux runtime testing
still outstanding. The proposal status table records every deferred item.

### Resulting behavior

- Viewer startup has a visible state card and a 15-second connection deadline.
  Failed connections retry after 1, 2, 5 and at most 10 seconds; Try again cancels
  the pending attempt. Old clients cannot overwrite a newer connection state.
  Expired sessions return to sign-in. A classic bootstrap catches module load
  failure and exposes Reload page even when `viewer.js` cannot initialize.
- Viewer controls float over a full-height remote desktop. View and Account use
  menus; controls hide after two idle seconds and stay visible while focused,
  hovered or a menu is open. Alt+Shift+V reveals them and focuses Clipboard.
- Sign-in has a mint primary action, accessible show/hide-password toggle,
  Caps Lock warning and actionable authentication/rate-limit messages.
- New and existing Servo views receive the effective light/dark appearance.
  The native theme suite checks website `prefers-color-scheme` through
  `matchMedia` during every theme switch and still checks page-state preservation.
- CPU/RAM is off by default and enabled through Settings → Advanced → Show
  performance in status bar. Existing profiles use the new default. Ordinary
  status/footer text no longer displays Servo/WebRender/version strings.
- Settings uses a centered modal sheet with constrained width and scrolling,
  dimmed backdrop, explicit Close, Escape/backdrop dismissal and restored focus.
  Opening diagnostics, site data, Downloads or a data-clear confirmation closes
  settings so the next surface is immediately operable.
  The tab-layout controls show the active selection; theme names are simplified.
- Site-data categories use Cookies, Local storage and Session storage. Rows put
  the site first and destructive Remove last; cookie headings use plain language.
  HttpOnly/Secure diagnostic fields remain available in the developer inspector.
- Windows builds without media show a startup warning; the website and README
  disclose the missing playback capability. This does not add Windows codecs.
- Confirmed external URI actions use ShellExecuteW on Windows and xdg-open on
  Linux, retaining macOS open. No shell command string is assembled from a URI.

### Verification and remaining limits

Evidence is recorded under `target/ui-ux-followup/` and
`target/coverage/javascript/`. The runtime overlay uses an existing Linux engine
binary with current gateway/viewer/runtime files; it is not a fresh Linux build.
Validated in this follow-up:

- 71 core/library tests and 33 desktop unit tests passed.
- JavaScript regression suite and its unchanged 100% line-coverage gate passed,
  including loader failure/deadline, retry backoff, stale clients, account rotation,
  toolbar focus and sign-in controls. See `target/coverage/javascript/lcov.info`.
- Native smoke, six-theme and capabilities suites passed with isolated profiles;
  the final theme/capabilities run also checks website appearance and settings
  Escape dismissal. Captures include `capabilities-settings.png`, site-data and
  all six theme frames. Settings and Paper captures were visually inspected.
- Docker HTTPS gateway regression passed using the local runtime overlay:
  authentication, private uploads, CSRF/Origin/size guards, nginx syntax, restricted
  listeners and current viewer/bootstrap assets. Disposable containers were removed.
- Chrome static-viewer visual checks confirmed the mint sign-in action, show/hide
  password control, floating controls/View menu and missing-module Reload card.
  This deliberately omitted noVNC to test loader failure; it does not certify a
  live remote connection or container restart through Cloudflare/Traefik.
- Test-harness desktop build, Clippy with warnings denied for application targets,
  formatting and diff whitespace checks passed. Vendored Servo warnings and the
  existing macOS linker unwind-size warning remain; they were not suppressed.

No commits, pushes, release publication or changes to the user's profile were
made. Real screen-reader tasks, production proxy/container restart recovery,
mobile viewer operation, Windows/Linux external-app dispatch, Windows startup
and playback, platform installers/signing and backdrop behavior remain open.

## Continued follow-up — 23 October 2026

- B3/B5: the connection indicator is an accessible button. HTTP explicitly says
  Not secure. Site data opens below this control with an encryption explanation
  and a current-site clear action using the existing destructive confirmation.
  The clear target uses Servo's stored registered-site name, not a URL origin.
  The panel closes and cached cookies clear on URL changes to avoid showing
  another page's cookies. Domain emphasis and permission-management UI remain open.
- B6/B7: the theme picker shows three-color swatches. The sidebar brand uses one
  compact row. Tab close glyphs appear for the active tab, pointer hover or
  keyboard focus; the close button stays keyboard accessible. Platform theme
  filtering and duplicate footer actions remain open.
- A4: explicit paste events on the remote screen send text; remote clipboard
  updates offer Copy here. Clipboard denial or a missing API opens selectable
  text for manual copying. No background local-clipboard reads are performed.
  Live noVNC shortcut delivery and mobile clipboard validation remain open.

Validation: test-harness build and JavaScript 100% line gate passed. Native smoke
and capabilities captures are under `target/ui-ux-followup-2/native/`.
These changes remain uncommitted alongside the previous follow-up.

## Third follow-up — 23 October 2026

Implemented About Wega, the new-tab shortcut grid and recent links with saved
PNG favicons/data-only image CSP, a library fallback icon, native-backdrop theme
filtering and removal of the duplicate sidebar bookmark control. New-tab links
are escaped and deduplicated. Search requests HTML autofocus; native keyboard
focus is not yet certified. The profile has unique recent visits, not visit
frequency counts, so this is not a most-visited ranking.

Network details use a bottom panel below 900 logical pixels. Default columns fit
available width where readable; manually sized columns retain horizontal scrolling.
Table paint is clipped to its own panel and scrollbars remain visible. Filter
syntax moved into a tooltip instead of consuming a permanent line.

Viewer Quality offers Sharp, Fast and Auto with optional local preference storage.
Auto follows data-saving/slow-connection hints when the Network Information API
exists; other browsers use balanced quality. It does not inspect noVNC's private
frame queue and is not claimed to implement frame-backlog adaptation.

`packaging/windows/wega.iss` defines a per-user installer with Start menu and
uninstall entries, HTTP/HTTPS and HTML capabilities, and quoted URI launch commands.
It does not overwrite UserChoice. Settings opens Windows Default apps explicitly.
CD compiles the installer and runs `scripts/windows-installer-check.ps1` to check
files, registry capabilities, the shortcut and uninstall. These Windows steps are
prepared but have not run on this Mac. No installer has been published or signed.
The existing portable ZIP remains available. A release signing identity, Windows
CI/runtime validation and URL handoff to an already running instance remain open.

Validation artifacts: `target/ui-ux-followup-3/native/`; JavaScript line gate,
workflow lint and release-script tests. Native smoke, capabilities and toolbox
runs exercise the changed surfaces. The final checks are recorded in those reports.

Still requiring implementation: direct viewer/chooser IPC handoff, mobile keyboard
and touch gestures, frame-queue quality adaptation, site permission management,
address-domain emphasis, Windows media/backdrop integration, macOS default-browser
registration and a confirmed engine fix for the Wikipedia icon issue. Production
proxy restart, screen-reader and real-device checks remain unverified.

Final third-follow-up checks: native smoke, capabilities and toolbox passed;
JavaScript **443/443 lines** passed the unchanged 100% gate; six release tests,
Clippy with warnings denied for application targets, actionlint, formatting and
whitespace checks passed. New-tab and Network captures were inspected. The
capabilities fixture retains its earlier 1.5× pinch zoom, so its new-tab capture
is a zoomed viewport rather than a default-zoom full-page image. Windows installer
compilation, installation and signing remain unverified here.

## Fourth follow-up — 23 October 2026

### Site permissions and address display

The site panel has persistent Block/Ask controls for common requested features,
scoped to the exact canonical HTTP(S) origin, including scheme and port. Blocking
also denies matching queued requests, without consuming requests for another site
or feature. Missing settings continue to prompt; grants remain Allow once.
The callback identifies the top-level view origin, not an independent requesting
frame origin, so remembered automatic grants are deliberately not implemented.
Blocks apply to requests from that view and its frames. Existing device access
is not revoked by this UI; close pages already using it when blocking access.
The panel explains this scope, scrolls within the viewport and closes on Escape.

The unfocused address field emphasizes the complete parsed host and mutes the
scheme, credentials, port and path. Editing uses uniform text. Host boundaries
come from URL parser positions, so a hostname in credentials or a path cannot
be highlighted as the destination. This is full-host emphasis, not PSL-based
registrable-domain extraction.

### Direct Docker file handoff

The native picker publishes a random, five-minute request in `/run/wega/chooser`.
The authenticated viewer offers Choose from this computer and accepts dropped
files while a request is active. It uploads through the existing bounded upload
endpoint, then submits the uploaded paths with the request ID. Multiple selection
is limited to 32 files of at most 32 MiB each; the existing profile quota still
applies. Path entry and the older Upload dialog remain available as fallbacks.

Chooser GET requires a session. POST also requires the expected HTTPS Origin and
CSRF token, checks the current ID and expiry, and only accepts regular files below
the upload root. The native side independently verifies ID, selection count and
canonical file containment. A first-terminal-action marker prevents late uploads
from replacing a cancellation. Picker completion/drop retires request artifacts;
the actual uploaded files follow the existing retention policy.

IPC uses a browser-owned setgid directory shared with the gateway UID's IPC group,
not an HTTP-served path or a new TCP listener. Manifests and replies use atomic
publication. A replaced picker cannot delete a newer request's manifest. Only
origin metadata is published, not full URL paths, query strings or credentials.

### Verification scope

- Core checks cover persisted blocks, exact-origin boundaries, legacy settings,
  spoof-resistant host ranges and one-time resolution of matching queued requests.
- Native smoke/capabilities checks exercise address input, remembered notification
  denial without a prompt, the site controls and a real Servo file input populated
  by a handoff reply. The page reads the expected selected-file bytes.
- Native handoff unit checks reject stale IDs and files outside the upload root.
- Gateway tests cover session/CSRF/Origin requirements, expiry, uploaded-file-only
  selection and terminal cancellation. The Python 3.14 environment supplies the
  existing cryptography dependency; socket tests require Unix listener access.
- The Docker HTTPS regression checks that the real browser UID can publish a
  request and read the gateway reply while gateway secrets remain private. It
  uses an older Linux engine binary with current runtime files, not a fresh Linux
  Rust build or a complete live chooser interaction through noVNC.
- Viewer regressions cover direct selection, multiple/size bounds, cancellation,
  stale/error responses, drag/drop and fallback behavior. The unchanged JavaScript
  100% line gate is retained. Full live Docker, touch-device and production-proxy
  validation remain required.

Reports: `target/ui-ux-followup-4/` and `target/coverage/javascript/`.
No commits or pushes were made. Mobile device validation, frame-queue quality adaptation,
platform media/backdrops/default-browser integration, signing and the engine icon
issue remain open, as do the previously listed hardware/release checks.

### Mobile keyboard follow-up

The viewer has a Keyboard control, an on-screen-keyboard input bridge and larger
coarse-pointer targets. Input differences send Unicode code points, corrections
and Backspace through public `RFB.sendKey`; navigation keys and common modified
shortcuts are forwarded explicitly. This avoids reading local clipboard content
or depending on noVNC's private fields. Focus-on-click is disabled while typing so
remote clicks can retain the local keyboard. The hint is dismissible and its
storage is optional. The existing noVNC implementation handles two-finger scrolling
and pinch as remote-page zoom; independent remote-viewport pinch zoom is not added.

Regression checks cover emoji/non-ASCII text, deletion, line breaks, modifier
release, focus behavior, disconnected input and hint dismissal. Real iOS/Android
keyboard display, IME/autocorrect, touch gestures and assistive-technology behavior
still require devices. These checks are not a mobile-device certification.

Final fourth-follow-up results: 74 core and 34 desktop unit tests passed; native smoke and capabilities
passed, including the permission toggle and real selected-file bytes. Gateway
suite: 32 tests passed. JavaScript: 557/557 lines passed the existing 100% gate.
Clippy, formatting, shell syntax and diff whitespace checks passed. Docker HTTPS
chooser IPC/UID regression passed; its disposable container and overlay image
were removed. No new Python line/branch coverage percentage is claimed here.
