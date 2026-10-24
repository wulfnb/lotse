# UI/UX implementation — 22 October 2026

All 16 findings from [the review](ui-ux-review.md) have implementation changes.
The original follow-ups were committed as `7d6b1bc`; later CI fixes and continued
work are recorded at the end. Commit statements in earlier checkpoints describe
those checkpoints, not the current Git state.
The [23 October follow-up](#23-october-follow-up) tracks the separate second-pass backlog.
The [24 October checkpoint](#remaining-proposal-implementation--24-october-2026) is the latest status; earlier remaining-work lists describe their checkpoints.
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


## CI corrections and continued work — 23 October 2026

[PR #4 run 39983191666](https://github.com/wulfnb/wega/actions/runs/39983191666)
failed for three separate reasons:

- The Docker builder omitted the package's `build.rs`. It now copies the script
  before Cargo runs; a release regression checks that the manifest's build script
  is among those build inputs.
- Core Rust coverage passed, but eight defensive chooser-gateway lines were
  uncovered. Added tests exercise oversized/malformed manifests, missing and
  non-file uploads, an atomic claim race and failed publication with safe retry.
  The gateway's 100% line threshold is unchanged. Direct invocation of the auth
  test module now includes chooser tests as well as unittest discovery.
- The instrumented macOS capabilities test queried the prior document during an
  offline navigation. It now waits for the blocked destination and retries the
  DOM predicate within a bounded deadline, retaining the actual offline assertion.

An additional Linux runtime check exposed an early-rejection race in the direct
Unix-socket auth test client: the gateway can return 403 before the client's
separate JSON body write. The test now reads and asserts the actual response on
`BrokenPipeError` and always closes the connection; no rejection assertion or
gateway guard was removed. Empty 204 replies also avoid a needless body write
after the client has received their headers; a regression simulates a closed
body stream and verifies that no write occurs.

Continued proposal work:

- Windows HTML associations now resolve native absolute drive and UNC paths to
  file URLs, including spaces, rather than sending them to web search. A
  Windows-only regression covers both forms; Windows execution remains required.
- New tab menu, plus button and Cmd/Ctrl+T share one search-page action. It clears
  address-field focus and routes typing to the page; completion focuses the
  search field only while the page still owns focus, so Cmd/Ctrl+L can interrupt.
  The native capabilities case sends key events to Servo and checks the resulting
  search-field value without clicking the page. It also checks that the browser
  routes typing to the page; this does not certify OS-level IME behavior.

These corrections were committed as `6256600` before the next follow-up; the agent has not pushed. At that checkpoint, whole-project
100% coverage, Windows runtime/signing/media/backdrops, macOS default-browser
registration, running-instance URI handoff, most-visited ranking, frame-queue quality adaptation,
independent viewer pinch/pan, the confirmed Wikipedia engine fix and the listed
real-device/assistive-technology/production-proxy checks remain outstanding.


Local verification for the CI follow-up:

- Fresh isolated core measurement: **1555/1628 lines (95.52%)**, 74 tests passed;
  the unchanged 95% gate passes on macOS. The failing Ubuntu run had already
  passed its Rust gate; its gateway coverage was the failure.
- Full Python discovery: **52 tests passed**, gateway **443/443 lines** and
  **158/158 branches**. The unchanged 100% line gate passes.
- Final instrumented desktop run: **76 library + 34 desktop unit tests passed**;
  all eight native suites passed (smoke, theme, capabilities, toolbox, download,
  WebDriver, media and capture). The capabilities run checks focused new-tab key
  input and the blocked offline document. Desktop line measurement is
  **10564/12716 (83.08%)**; whole-project 100% remains outstanding.
- The new-tab capture was inspected; it retains the fixture's deliberate 1.5×
  pinch zoom. This is not a default-zoom full-page or real-device certification.

Artifacts: `target/ci-fix/`, `target/coverage/rust/`,
`target/coverage/python/` and `target/coverage/desktop/`.


Fresh Linux ARM64 Docker verification:

- Built the current Rust sources with `BUILD_PROFILE=dev` and `test-harness`;
  this checks the missing-build-script fix in a fresh Linux build. License-notice
  generation was disabled for this disposable debug image.
- Authentication/session rotation/WebSocket revocation, rendering smoke, native
  downloads, HTTPS gateway/chooser UID/security checks and all shell contracts
  passed. Current gateway/viewer sources were mounted for the HTTPS regression.
- At this checkpoint toolbox **failed** at the Memory probe: the process aborts in pinned Servo's
  `servo-script-bindings-0.7.0/mem.rs:19` with `attempt to add with overflow`.
  This was reproduced on both fresh ARM64 debug builds. The root cause is not
  isolated; no test assertion or overflow check was disabled. Linux release and
  x86_64 impact are unconfirmed, and GitHub's Ubuntu release job has not rerun.
  See [the capability matrix](servo-capabilities.md). A complete Linux toolbox
  pass and fully green cross-platform CI are not claimed.
- Disposable test containers/networks were removed. The newer gateway empty-204
  write guard is covered by Python tests; the full Docker image predates that
  Python-only guard, while its native binary includes the final UI changes.

Linux evidence: `target/ci-fix/docker-final-build.log`,
`target/ci-fix/linux-final-checks.log` and `target/ci-fix/linux-container.log`.
The remaining UI and platform backlog is still open; this checkpoint does not
mark the entire proposal complete.


## Allocation reports and remaining UI work — 23 October 2026

The prior CI corrections are saved in `6256600`. This follow-up fixes the Linux
Memory failure and continues the separate proposal backlog.

### Real allocation-report fix

Pinned Servo generated a Box-style size callback for Rc-backed DOM classes.
The pointer returned by `Rc::into_raw` addresses the value after the allocation's
reference-count header; passing it to the allocator's size API is invalid. This
caused the Linux ARM64 debug overflow during the real toolbox Memory probe.

The published `servo-script-bindings` 0.7.0 crate is now vendored. Its generator
selects the correct callback for each allocation kind. Rc-backed objects acquire
a temporary strong reference and use Servo's existing Rc measurement trait to
measure the allocation base and contained values; the temporary reference then
drops. Box-backed objects keep their original path. No arithmetic checks or
Memory assertions were disabled, and no installed Cargo source was changed.

Two Rust regressions check aligned Rc allocation pointers, preserved ownership
counts and the Box path. A Python regression executes the actual DOM vtable
generator for both kinds. See [the vendor patch notes](../vendor/README.md).
Coverage fingerprints now include embedded JavaScript and vendored source and
generator changes, preventing stale reports from validating a changed engine.

A fresh Linux ARM64 debug/test-harness build passes both allocation regressions.
The resulting image passes authentication/session rotation/WebSocket revocation,
rendering smoke, the full toolbox including Memory reports, downloads, current
HTTPS gateway/chooser security and UID checks, and all shell contracts. Test
containers/networks were removed. Evidence: `target/ci-fix/rc-linux-checks.log`,
`linux-rc-final-build.log` and `linux-container.log`. GitHub Ubuntu release CI has
not rerun; this is local ARM64 evidence.

### Most-visited shortcuts

History entries persist a saturating visit count. Saved bookmarks stay first;
remaining shortcut positions use frequency, with recency as the tie breaker,
URL deduplication and the existing eight-tile limit. The recent-links section
retains chronological order. Old profiles default missing counts to unknown and
retain their order until new visits establish frequency. Counts live only in
bounded history entries, so history deletion or eviction removes them too.

Regression tests cover profile migration/round-trip, repeated visits, overflow,
frequency versus recency, bookmark precedence, deduplication, limits and history
clearing. The native capabilities fixture checks a genuinely ranked shortcut in
the rendered document as well as new-tab keyboard input.

### macOS browser registration and launch events

The app bundle declares HTTP/HTTPS viewing and HTML/XHTML document associations,
with an Alternate document-handler rank. Browsing settings opens System Settings
and explains where to choose Wega. Installation in Applications and the user's
explicit default-browser choice remain required; no default was changed.

Dedicated Apple-event handlers forward web URLs and local document URLs to new
tabs in the existing browser. Early events are queued until initialization.
Handlers are registered again after AppKit startup, preserving winit's own
application delegate. URL strings are decoded separately from file descriptors:
Cocoa's file coercion can reinterpret an HTTP string as an HFS filesystem path.
Only HTTP, HTTPS and file schemes are accepted, with per-address and batch limits.

`scripts/macos-launch-test.py` creates a uniquely identified disposable app and
profile, launches a real cold-start HTTP event, then sends two URLs and an HTML
file containing spaces, an ampersand and an accented character to the running
app. WebDriver checks loaded titles/URLs, tab counts and the unchanged process.
The test normalizes macOS's decomposed Unicode filename representation. Cleanup
terminates only that executable and unregisters only the disposable test app.
The desktop coverage runner includes this test on macOS and preserves failure
status. Installer/default-browser selection on real machines, Windows instance
handoff, Developer ID signing and notarization remain separate release checks.

### Remaining proposal work

Implementation still open: frame-queue-driven viewer quality adaptation;
independent viewer viewport pinch/pan; bundled Windows media and per-element
fallbacks; a native Windows backdrop; Windows running-instance URL handoff; the
confirmed Wikipedia icon engine fix; and remembered permission grants/current
access revocation once the engine exposes sufficient origin/access information.

Validation/release work still open: Windows build/installer/runtime and physical
PC checks; real iOS/Android keyboard, IME, gestures and clipboard tasks; screen
reader and OS accessibility settings; a real OS appearance toggle and compositor
checks; production-proxy restart recovery; signing/notarization credentials; and
complete cross-platform whole-project coverage. Core/gateway component gates do
not establish whole-project 100% or a fully finished browser.


### Final local verification for this follow-up

- Core: **77 tests passed**, **1591/1664 lines (95.61%)**; the 95% gate passes.
- Python: **55 tests passed**; gateway **443/443 lines and 158/158 branches**.
  The unchanged 100% line gate passes.
- Instrumented desktop: **79 library + 34 desktop + 2 allocation tests passed**.
  All eight native suites pass: smoke, theme, capabilities, toolbox, download,
  WebDriver, media and capture. Real cold-start URL and same-process batched URL
  plus Unicode HTML-file Launch Services delivery also pass.
- Desktop measurement: **10721/12888 lines (83.19%)**. Whole-project 100% and all
  required platform reports remain outstanding. Coverage inventory is refreshed;
  report completeness has not been claimed or its policy weakened. The new
  Launch Services regression is classified with the other test drivers.
- Production and test-harness Clippy, formatting, package-script syntax and
  first-party/modified-vendor-line whitespace checks pass. The imported archive
  retains its original upstream whitespace; only its two documented files differ
  from the published crate. Existing vendored Servo and macOS linker warnings
  remain. The new-tab capture was inspected; the fixture still uses its deliberate
  1.5× pinch zoom and does not certify default-zoom or device layout.

Evidence: `target/ci-fix/{core-followup-final,python-followup,desktop-followup}.log`,
`target/coverage/{rust,python,desktop}/`, and desktop native run
`run-hrdawz1d/` (including `macos-launch/launch-l7eljkqf/result.json`). The fresh
Linux evidence is listed above. The agent has not pushed or changed the user's
default browser. GitHub's Linux x86_64 release and Windows checks must run after
push before cross-platform CI can be called green.


## Remaining proposal implementation — 24 October 2026

### Viewer backlog and local gestures

Auto quality samples actual queued actions, flip frames, oldest-action age and
completed paints. Two congested samples lower quality; ten clear samples with new
painted frames recover it. Idle connections never improve quality. Network hints
can cap quality, while manual Sharp/Fast stay explicit. The Docker build validates
the pinned noVNC source hashes before adding a small public statistics/viewport
API; changed upstream source fails the build before any patch is written.

Pan/zoom viewer is a separate local mode: remote input and remote resizing pause,
one pointer pans, two touch pointers zoom around their centroid, and Reset fits
the existing framebuffer. Leaving the mode restores remote input and the selected
size preset. Capture-phase handlers consume gestures only in this explicit mode.
The viewer stylesheet had inadvertently contained HTML; it is restored to CSS,
including compact/coarse-pointer controls, hidden states and the viewport origin.

The real-browser regression starts an authenticated disposable Docker viewer and
isolated Chrome. It checks real desktop pixels, touch pinch/pan, preserved zoom on
resize, Fit restoration and unchanged remote resolution while panning. A real
queued SVG decode/flip, followed by actual paints, proves backlog lowering and
recovery; the frame queue drains and the expected red pixel is visible. No test
credentials or Chrome profile are retained in uploadable artifacts. Physical
mobile devices and their keyboard/IME behavior remain separate checks.

### Windows implementation and media fallback

CI and release builds enable media using official GStreamer MSVC x64 1.28.8,
verified against a pinned SHA256. Packaging parses normal and delayed PE imports
and copies only Servo's explicit plugins and their dependency closure, retaining
license texts and provenance/hashes. Required Microsoft VC runtime DLLs come from
the installed Visual Studio release x64 REDIST CRT directory, with separate
notices and file hashes. A runner’s System32 copies cannot hide missing packaged
VC dependencies. Missing dependencies, architecture mismatch
and excluded FFmpeg/x264/x265/FAAC dependencies fail packaging. Windows adds native
D3D11/Media Foundation plugins. Development SDK discovery checks actual plugin
presence; packaged plugins take precedence. The published preview has not been
replaced by these changes.

Per-element video/audio failures show accessible network or unsupported-media
messages with a download action only for safe HTTP(S) sources. Source reload
clears obsolete messages. Native WebM/WAV decoding and the error fixture pass on
macOS. The Windows CI regression strips SDK/plugin environment paths before
checking packaged playback, then sends HTTP and Unicode/space HTML paths to a
running profile and verifies loaded tabs and the original process.

Windows launch handoff uses a per-profile named mutex and message-only window,
validates bounded HTTP/HTTPS/file messages and sends work onto the UI event loop.
DWM Mica is gated by Windows API support, high contrast and the user's transparency
setting; unsupported or failed configurations retain opaque rendering. Both real
Win32 adapters typecheck against the Windows target. Actual MSVC build, standalone
media playback, installation, Mica composition and OS preference changes still
require Windows CI and a real PC; no Windows runtime success is claimed locally.

### Wikipedia diagnosis and scoped fallback

The captured page reported no mask-image or -webkit-mask-image support. Its
Vector stylesheet uses SVG masks over a black background, explaining the black
squares. A scoped compatibility script reads same-origin MediaWiki load.php CSS
and renders its original same-origin SVGs as backgrounds on matching vector-icon
selectors. It preserves direction/appearance selectors, skips engines with mask
support and does not change unrelated page rules or substitute guessed glyphs.
A native fixture using this mask pattern verifies both a white transparent corner
and the black glyph center in an actual screenshot. Fresh live Wikipedia
navigation timed out during verification, so a corrected live-page capture is
still required. General Servo CSS-mask rendering remains an engine gap.

### Permission engine dependency

Approval now says “Allow for this page”; site controls explain that Block affects
queued/new prompts and cannot stop existing device access. Remembered automatic
grants and active-access revocation are not implemented. The pinned callback
lacks trusted requesting-frame/document identity, permission revocation is empty,
and the media track implementation lacks stop. See the exact source references,
required engine contract and iframe/worker/track acceptance checks in
[permission-engine-requirements.md](permission-engine-requirements.md).

### Remaining checks

Windows runtime and physical-PC checks; live Wikipedia verification; real
mobile devices, IME, clipboard and accessibility; real appearance/high-contrast
changes; production-proxy recovery; signing/notarization; complete cross-platform
coverage; and the permission engine work above remain outstanding. Earlier
checkpoint lists are superseded by this section, without turning unrun platform
checks into completed work.


### Verification for the 24 October checkpoint

- Core: **77 tests pass**, **1591/1664 lines (95.61%)**, passing the 95% gate.
  The Windows drive-less-path test expectation is fixed separately in `0674e8e`;
  all core tests compile for the Windows GNU target. Actual Windows execution is
  left to the configured Windows CI job.
- Desktop: **79 library, 34 desktop and 2 allocation tests pass**. All eight native
  suites and macOS cold/running-instance URL and Unicode HTML launch checks pass
  against the final embedded scripts. Coverage is **10769/12933 lines (83.27%)**.
  The permission-prompt harness now expects its truthful “Allow for this page”
  label; no permission assertion was removed.
- Python: **60 tests pass**; gateway coverage remains **443/443 lines and 158/158
  branches**. PE import/dependency, fail-closed noVNC patch and media regressions
  are included. JavaScript’s unchanged **100% line gate passes**.
- Final viewer assets pass real authenticated Chrome/noVNC checks, including the
  centered login surface without forced clipping, actual queued decode and painted pixels,
  touch gestures, resize preservation and Fit restoration. Reconnection keeps
  remote resizing paused in local pan mode, including with the Resize preset.
- The fresh Linux ARM64 image passes auth/session/WebSocket checks, smoke,
  toolbox/Memory, downloads, gateway/upload/chooser security and shell contracts.
  The final viewer layer is separately exercised in Chrome. This is local ARM64
  validation, not a GitHub Ubuntu x86_64 CI result.
- Production/test-harness Clippy with application warnings denied, formatting,
  workflow lint, whitespace checks and real Win32 adapter cross-compilation pass.
  Existing vendored Servo warnings remain. Actual Windows SDK installation,
  MSVC linking and media/backdrop/handoff runtime are unverified locally.
- Whole-project 100% is **not achieved**: the refreshed inventory still has 46
  first-party files incomplete/unmeasured and four missing platform configurations.

Evidence: `target/remaining/{desktop-verified,core-final,python-final,linux-final,
viewer-final,windows-adapter-final}.log`, `target/coverage/desktop/native-tests/
run-7zdr7sf4/`, and `target/viewer-live/run-TZ6hXj/`. Raw profiles and screenshots
remain local; temporary test containers were removed. No push was performed.
