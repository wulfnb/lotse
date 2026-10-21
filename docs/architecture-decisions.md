# Architecture decision record

The accepted model is a modular monolith with ports and adapters. The application
is one process and one Cargo package; dependencies point from desktop adapters
toward the reusable core. Splitting into crates is deferred until a real boundary
requires separate compilation or distribution, as proposed in the initial review.

| Original recommendation | Implementation | Validation |
| --- | --- | --- |
| Extract an engine-independent core | `lib.rs`, optional desktop dependencies, unsafe forbidden in core | Default-free core tests and Clippy |
| Use commands and events for core changes | Application commands/events; session commands/events; navigation commands | Pure transition and port contract tests |
| Separate persisted state from presentation | Application owns private Profile; Chrome owns ephemeral forms/input | Compilation prevents UI profile mutation; settings and native smoke suites |
| Narrow Browser responsibilities | Browser coordinates core operations and owns desktop handle lifetimes; workers, persistence, session transitions, permissions and presentation state have dedicated owners | Core contracts plus native integration |
| Encapsulate state behind operations | Profile and session structure are private; UI, shortcuts, WebDriver and native drivers use shared operations | Core and real WebDriver/tab-drag tests |
| Introduce replaceable external boundaries | ProfileStore injected into Application; NavigationPort, DownloadPort and consuming PermissionPort have concrete adapters | Failure propagation and zero-effect rejection tests with recording adapters |
| Keep crash-safe storage and recovery separate | FileProfileStore owns lock and guarded durable replacement; Application independently blocks writes after failed load | Lock contention, failed writes, corrupt files, stale temp files, collision saturation |
| Keep native tests out of production | Explicit test-harness feature and explicit isolated profile requirement | Parser contracts and native runner |
| Separate portable auth tests from Docker | Real handler tests use temporary credentials and substituted process boundaries; isolated Compose environment tests actual gateway/WebSocket behavior | Python contracts and Docker integration workflow |
| Automate quality and produce reports | Pinned tooling, CI workflows, native runner, Rust/Python/JavaScript exports, source inventory and exact 100% line gates | Gate tests reject rounding, missing files/platforms and invalid captures |
| Establish Git baseline | Initial commit c0849c1 appeared in the shared checkout during verification | This agent did not stage, commit or push; history left untouched |

The interfaces are intentionally specific. Rendering, AppKit objects, Servo
JavaScript/developer-tools requests and OS clipboard calls remain desktop adapter
implementation details. Introducing traits for every concrete method would add
indirection without improving the application boundary. Native test drivers can
inspect adapter details for integration assertions; application changes follow
the same operations as interactive use.

Remaining verification obligations are tracked in [quality-report.md](quality-report.md):
100% line coverage has not been achieved across Rust, shell files or all required
build configurations. Windows directory durability, physical permissions and
remote CI execution are not certified by macOS tests. These are open checks,
not completed architectural results.
