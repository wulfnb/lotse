# Coverage progress — 22 October 2026

The whole-project **100% line target remains outstanding**. Passing tests and
the existing component regression gates must not be described as achieving that
target. No thresholds were reduced and no production files were reclassified as
tests to conceal missing coverage.

## Verified measurements

| Scope | Covered / executable lines | Result |
| --- | --- | --- |
| Rust core, macOS ARM64 | 1256 / 1304 | 96.32% |
| Rust desktop, macOS ARM64 | 9649 / 11489 | 83.98% |
| Python production utilities | 400 / 400 | 100% |
| JavaScript production files | 328 / 328 | 100% |
| Production shell scripts | 77 / 77 | 100% in Linux process contracts |

The scopes overlap: Rust core is part of the desktop report. Percentages must not
be averaged. Python line counters are statement counts; its separate branch
metrics are informational. The desktop denominator includes first-party native
adapters, UI and engine integration, while vendored dependencies and explicit
test drivers are listed separately by `coverage-policy.json`.

The current desktop measurement has 1840 uncovered executable lines,
and the strict inventory lists 31 first-party files below target. All eight native
suites passed under instrumentation: smoke, theme, capabilities, toolbox,
download, WebDriver, media and capture. The source fingerprint matches this
checkout; the manifest records no failed tests.

## Added regression evidence

- 89 Rust unit tests: 56 core/library and 33 desktop tests. New cases cover input
  translation, structured console values, cookie inspection, debugger source
  search limits, network sorting/filtering/replay and malformed HAR imports.
- Download tests serve invalid ranges and changed validators through real
  loopback HTTP/curl transfers, verify safe single-connection fallback and compare
  final bytes. Additional tests cover retained segments, resource limits and
  clearing finished jobs without removing active/paused entries.
- Native contracts use a real Browser/Servo and AccessKit widget actions. They
  assert settings changes, destructive confirmations, user-script lifecycle,
  HAR import/export, console actions, debugger search/watches, tab shortcuts,
  focus switching, zoom, IME transitions, history confirmation and accessible
  bookmark removal. Bookmark removal is also checked after reloading the saved
  profile.
- The 83-stage capabilities suite additionally checks permission denial and
  acceptance, Basic authentication cancellation/sign-in, file-path validation,
  select/color controls and cancellation restoration. Notification permission is
  enabled only for the test-harness build; the release preference is unchanged.
- 33 Python tests pass. JavaScript line coverage passes for seven files. Shell
  contracts run four production scripts in a disposable container using actual
  kcov counters. Compiler/media/service boundaries are test ports; these contracts
  do not certify real macOS packages or a freshly built Linux application.

The native accessibility activation test exposed a real deadlock: reading egui's
pixel scale inside its accessibility node-builder callback attempted to acquire
the context lock again. Reading the scale before entering that callback fixes
the lock ordering; actual engine subtree activation now passes.

## Coverage infrastructure

- The Rust compiler wrapper is a host executable, including on Windows, with
  explicit LLVM executable suffixes. macOS continuous-profile alignment and
  optional branch instrumentation are preserved.
- Shell measurements carry collector counters, source hashes and test status.
  Stale/unverifiable Rust and shell measurements are rejected rather than shown
  as coverage for changed source files.
- CI collects macOS/Windows core reports and Linux shell reports. These workflow
  changes have been linted locally; remote CI has not run for this working tree.
- HTML, JSON and LCOV artifacts are generated under `target/coverage/`. Run
  `python3 scripts/coverage-inventory.py --require-complete` for the strict audit;
  its JSON/Markdown output lists uncovered lines and missing configurations.

The existing component regression gate, production/harness Clippy with warnings
denied, production build, formatting, workflow lint and shell lint passed. An
older optional branch report is preserved under `target/coverage/history/` and
is not presented as a measurement of the current checkout.

## Remaining work and validation limits

Native adapters and UI paths still have uncovered lines. The core download
service also retains uncovered failure/race paths. These are not silently
excluded. Consult the generated inventory for exact current line numbers.

Required Linux ARM64 desktop, Linux x86_64 desktop, Windows x86_64 core and
Windows x86_64 desktop measurements remain missing locally. The existing Linux
container used for shell/security contracts does not supply current Rust desktop
coverage. Neither a cross-compile nor passing macOS tests substitutes for those
platform runs.

Interactive computer-control permission was unavailable during this run. Native
OS file chooser/open/reveal behavior and full VoiceOver/NVDA tasks remain manual
validation items; simulated widget actions do not certify them. The isolated
manual test app and fixture server were stopped without using a personal browser
profile.

All changes remain uncommitted; nothing was pushed.
