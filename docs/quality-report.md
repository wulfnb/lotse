# Architecture and quality implementation — 21 October 2026

This is a historical implementation checkpoint. See the newer
[coverage progress report](coverage-progress.md) for current counters, added
regression tests and outstanding platform verification, and
[UI/UX implementation](ui-ux-implementation.md) for the subsequent UI changes.

No staging, commit or push was performed by this agent. An initial commit
`c0849c1` appeared in the shared checkout during verification; it was not created
by this agent and has been left untouched. The latest workflow, coverage-policy
and report edits remain unstaged and uncommitted.

## Implemented

- Extracted a core library that builds/tests without Servo, GStreamer or a GPU.
  Persisted settings, URL handling, host policy, session transitions, startup
  validation, download workers and storage now have explicit ownership.
- Session commands control tab selection/order through private model fields.
  Desktop input, WebDriver and engine callbacks use the same session state.
- Introduced the `ProfileStore` port and a filesystem adapter that owns its lock,
  blocks writes following a failed load, synchronizes replacements and protects
  committed profiles from stale/interrupted temporary files.
- Added malformed-setting normalization, invalid-favicon rejection and bounded
  restoration. Bookmark toggles are core operations.
- Separated download dialog state from the worker service; resume admission errors
  return to the UI explicitly. Added saturation/resume failure-path verification.
- Required `test-harness` and an explicit profile for every native test flag,
  including downloads. Rejected conflicting modes before startup side effects.
  Production builds omit the native drivers and capture hooks.
- Added shared WebDriver command deadlines and cancellation of undispatched jobs.
  Corrected readiness reporting so clients wait for initial page completion.
- Added portable gateway unit tests against the real handler, using temporary
  credentials, no sockets and no WebSocket/VNC dependency.
- Added pinned Rust/Python tooling, CI configurations, an isolated Docker quality
  environment, native suite runner, coverage exports and a coverage regression gate.
- Documented architecture, ownership, test commands, coverage scope, native safety
  invariants and validation boundaries. Temporary outputs and credentials remain
  ignored by Git; line-ending rules support cross-platform checkouts.

## Completed architecture follow-through

The later refactor implements the application boundary described in the original
review. See [the decision record](architecture-decisions.md) for each recommendation.

- `Application` now owns a private profile and an injected `Box<dyn ProfileStore>`.
  UI, native drivers and other desktop code can only read that profile; settings,
  bookmarks, visits, history and favicon updates use explicit commands/events.
- Application-level load protection independently prevents saving recovered
  defaults through an incorrectly permissive replacement adapter.
- Desktop session structure is private. Presentation and automation get read-only
  session access and use add/select/close/move operations for transitions.
- Shared navigation commands validate intent and call `NavigationPort`; a Servo
  adapter handles load, reload, history traversal and zoom. Native regression
  drivers now use those operations too, rather than bypassing them.
- `DownloadPort` separates application policy/admission from the concrete worker.
  Cookie translation and download coordination live outside widget rendering.
- Permission requests use a private FIFO and consume `PermissionPort` responders
  once. Request IDs reject stale UI responses, and shutdown/drop denies pending
  requests. This is covered without invoking physical device permissions.
- `Chrome` owns ephemeral input, focus, form and library presentation state.
  Browser remains the desktop coordinator for engine/window/GPU lifetimes.
- Expanded WebDriver regression coverage to navigate between documents, go back
  and forward, refresh, canonicalize internal aliases and reject invalid schemes.
  It exposed premature completion from unrelated traversal callbacks; completion
  now follows a core state machine and matching traversal IDs after engine events.
- Fixed restored active-tab selection when preceding `data:` tabs are omitted
  from a saved session. Regression tests assert selected-tab identity.

Application, navigation and permission core modules each measure 100% lines.
The concrete Servo adapters and older desktop code still have uncovered paths;
completing an architectural boundary is separate from completing coverage.

The new dynamic-port runner exposed fixed-port assumptions in the capability and
toolbox fixtures. Those assumptions were corrected and both suites passed their
reruns. The media suite exposed the premature readiness signal; real playback
passed after correcting the signal and waiting for readiness.

## Current coverage target and result

The user requires **100% line coverage across first-party production code**.
Branch results are informational. This target is enforced, but **has not been
achieved for the whole project**. The component and completeness gates currently
fail; they must not be described as passing or bypassed for a green build.

| Production scope | Covered / measured lines | Line coverage |
| --- | --- | --- |
| Rust desktop application, including the core | 8,021 / 10,740 | 74.68% |
| Rust core subset (not additive to desktop) | 1,247 / 1,304 | 95.63% |
| Authentication gateway | 249 / 249 | 100% |
| macOS media packaging utility | 79 / 79 | 100% |
| Bootstrap credential utility | 28 / 28 | 100% |
| All six standalone JavaScript files | 273 / 273 | 100% |

JavaScript covers `login.js`, `viewer.js`, `console_eval.js`, `inspector.js`,
`page_actions.js` and `reader.js`. Embedded function bodies are evaluated under
the same function-body contract as the browser, preserving original line numbers.
DOM/RFB dependencies are test doubles; real native suites remain a separate check.
Rust string-embedded probes, native adapters and UI handlers still have the gaps
shown by their owning files in the inventory. No aggregate whole-project
percentage is asserted while executable shell files and other builds are unmeasured.

`target/coverage/inventory.md` and `inventory.json` list every first-party source
file, its classification, exact counters and uncovered line numbers. The current
inventory has **38 production files below target or unmeasured**, including three
shell scripts. Linux ARM64/AMD64 desktop and Windows core/desktop measurements
are missing (four build configurations). Windows desktop is required because the
release workflow now builds a desktop executable, rather than packaging a core-only build.
Coverage tooling and test drivers are listed separately, rather than counted as
production. Vendored dependencies are outside this first-party target.

## Added regression coverage

- Startup validation in both enabled and disabled build configurations; the
  parser now uses one concrete validation state machine rather than duplicating
  it for every iterator type.
- Profile read failures, invalid URLs, local-file behavior, dated history,
  bounded favicon restoration, temporary-name saturation, and root-path rejection.
- Session transitions across payload types, content replacement, invalid commands,
  selection preservation, clearing and reopening.
- Real HTTP transfer failures, invalid/legacy metadata, destination collision
  exhaustion, cleanup after publication failures, and admission/resume limits.
- Gateway bootstrap commands, encrypted password files, process shutdown races,
  watchdog expiry, server teardown and silent logging. Test doubles prevent real
  signals, subprocesses, listeners or account changes during unit tests.
- JavaScript login/session failures, reconnect/stale-client behavior, clipboard,
  fullscreen errors, download listing, account changes and CSRF, signout,
  getter-safe console previews, CSS/DOM inspection, bounded find and reader mode.
- Release helpers against a temporary SDK/application. These tests verify that
  installed libraries are never modified or signed. They exposed and fixed an
  eager `pkg-config` lookup even when a plugin-directory override was supplied.
- Coverage control tests reject rounded 99.9999%, empty/corrupt reports, omitted
  files and missing platforms. A real instrumented Rust probe verifies nonzero
  counters survive process termination on Darwin.

## Latest local verification

| Verification | Result |
| --- | --- |
| Full Rust unit suite with harness | 67 passed: 52 library + 15 desktop |
| Instrumented core suite | 50 passed |
| Python suite | 28 passed |
| JavaScript suite | 16 passed; all six source files at 100% lines |
| Native smoke, themes, capabilities, toolbox, downloads, WebDriver, media | All seven passed with instrumentation |
| Strict application Clippy and formatting | Passed |
| Production build | Passed; normal uninstrumented binary restored |
| Exact 100% component gate | **Fails correctly: core has 57 uncovered lines** |
| Whole-project completeness gate | **Fails correctly: remaining code and platforms are not covered** |

One filesystem test failed once during concurrent measurement, then passed eight
isolated reruns and fresh complete suites. Temporary download-test directories
now include an atomic counter in addition to process/time IDs. The original
failure's root cause was not proven; this is not a claim that every potential
filesystem flake has been eliminated.

## Final build, navigation and authentication fixes

- Removed the duplicate Servo history callback while preserving history snapshots
  and profile/document change notifications. Production and harness compile again.
- History traversal completion now matches both the request ID and the snapshot's
  destination. WebDriver title reads query the active document rather than stale
  embedder metadata. Cached refreshes use Servo's synchronous Started transition;
  ordinary loads still wait for their actual load progress.
- The expanded real WebDriver suite passed internal URL aliases, rejected schemes,
  new-document navigation, back/forward, refresh, end-of-history, title/source,
  script results, screenshots and session deletion.
- Authentication writes use private exclusive temporary files and clean them on
  failed publication. Predictable temporary symlinks cannot truncate another file.
- Non-ASCII CSRF headers are compared as bytes; invalid Unicode passwords produce
  a validation error. Both cases have regression tests against the real handler.
- Existing changes to superseded navigation handling, failed-save state rollback,
  curl config isolation and filename safety were retained. Failed-save rollback
  now has explicit assertions for the in-memory URL/selection state.
- Rust reports carry a source fingerprint. Export/inventory rejects changed or
  unverifiable sources, and empty platform reports cannot satisfy required builds.
  Windows report paths are interpreted correctly when collected on another OS.
- Release packaging now depends on the reusable CI workflow. Failed tests or
  required coverage gates block release jobs, including Docker publication.
  Called CI and standalone CI use separate concurrency groups so they cannot
  cancel each other. Release tags are passed through environment variables and
  validated as Git refs, checksum output no longer includes itself, and Windows
  packaging refuses archives without a real desktop executable. All workflows
  passed actionlint; local release-tag and checksum regressions passed. No remote
  workflow, cross-platform package or release was triggered here.

The final native results are in
`target/coverage/desktop/native-tests/run-lk1wdzkp/results.json` (all seven passed).
Current desktop/core measurements have `tests_passed: true` and matching source
fingerprints. This supersedes the earlier failed WebDriver runs and stale metrics;
those earlier results were not evidence that the final checkout passed.

See [the focused authentication security fixes](security-fixes.md). This was not
an exhaustive security audit of Servo or the complete application.

## Measurement and validation boundaries

Rust workspace-only instrumentation leaves dependencies uninstrumented. Native
child processes are measured too. Darwin uses continuous profiles with aligned
counter/data/bitmap sections: without alignment, LLVM rejected continuous capture
and terminated WebDriver processes lost their counters. A subprocess regression
test now detects that problem. Instrumented application objects are retained
locally for source-checked re-export even after the ordinary executable is rebuilt; the large
object snapshot is excluded from CI artifact uploads.

Branch measurements used the installed Rust 1.99 nightly compiler for the core;
production/desktop instrumentation uses pinned Rust 1.98.0. Branch percentages
are not part of the requested line-coverage gate. Python was tested with 3.15,
Node with 26; CI's Python 3.12 remains unverified locally.

Remote workflows have not run because nothing was committed or pushed. Native
coverage is from this macOS ARM64 host. Physical-device behavior, OS permission
grants, other feature combinations and rebuilt Linux images have not been
certified by this run. The previous disposable Docker gateway/WebSocket test
remains behavioral evidence, not a Rust or shell line-coverage measurement.

Two existing vendored Servo warnings and the native linker unwind-section warning
remain. Application lints pass with warnings denied. Defensive error handling,
platform paths and UI callbacks were not excluded to manufacture 100%.


## 24 October UI continuation

The latest successful local measurement covers 79 library, 34 desktop and two
DOM allocation tests, all eight native suites and real macOS launch-event tests.
Desktop line coverage is 10769/12933 (83.27%); core is 1591/1664 (95.61%). Python’s
60 tests pass and the gateway retains 443/443 lines and 158/158 branches. The
JavaScript 100% line gate passes with the new backlog and viewport controllers.
The inventory still reports 46 first-party files incomplete/unmeasured and four
missing platform configurations. Windows adapter cross-compilation is not a
Windows runtime/coverage report. See the [current UI checkpoint](ui-ux-implementation.md#remaining-proposal-implementation--24-october-2026)
for artifacts, Linux/Chrome integration checks and explicit remaining work.
