# Engine strategy: a low-memory Wega engine

Status: plan, not implemented. Written 23 October 2026 to start the work; the
open decisions at the end are for the project to make together.

## Why

On a home server, Wega in Docker used about 500 MB per busy page, returned only
part of it after tabs closed, peaked at 1.998 GB against a 2 GB limit, and kept
both of its CPUs busy with software rendering. Firefox on the same site used
about as much memory, so this is not a Wega bug, but it is the cost of an
engine designed for maximum compatibility and speed, not for small memory.

Goal: an engine Wega owns, that is **low on memory and CPU first**, **supports
JavaScript**, and grows towards **real-site compatibility**, without breaking
the sites that work today while it grows.

## What we found

### Servo's DOM cannot be reused without SpiderMonkey

Reading Servo 0.7's `script`, `script-bindings` and `layout-api` crates:

- Every DOM node is a SpiderMonkey object. `Node` starts with an `EventTarget`
  holding a `Reflector`, and the Rust struct is freed only when SpiderMonkey's
  GC finalizes its JS object. There is no layer that abstracts the JS engine;
  691 of 883 `script` source files use `js::` types directly.
- Reusing it means carrying about 296,000 lines of `script`, 574 WebIDL files
  with a 43,000-line Python binding generator, and SpiderMonkey itself. Only
  Servo's layout and WebRender painting could be dropped.
- Servo's layout boundary (`LayoutFactory` / `Layout`, about 40 methods) is
  pluggable in principle, but its configuration bakes in Servo's fonts, image
  cache and WebRender paint API, and the real implementation reads the DOM
  through raw node addresses from the `script` crate.
- Servo runs SpiderMonkey with incremental and compacting GC **off** (both
  marked as unsafe with Servo's DOM) and all JIT tiers on. This plausibly
  explains part of the slow, partial memory release seen in practice.

Decision: **no SpiderMonkey and no Servo DOM in the new engine.** Servo stays
only as the existing, full-compatibility engine.

### Blitz is the right base

[Blitz](https://github.com/DioxusLabs/blitz) (DioxusLabs, `0.3.0-beta.4`,
`MIT OR Apache-2.0`) already assembles the Rust parts a modern engine needs:

| Part | Blitz | Notes |
| --- | --- | --- |
| HTML parsing | `blitz-html` on html5ever 0.40 | same family as Servo 0.7 |
| DOM | `blitz-dom` (29,000 lines) | nodes in a `SlotMap` with generational ids; `DocumentMutator` for changes |
| CSS | Stylo 0.22 (pinned fork of upstream main) | the same style engine as Servo and Firefox |
| Layout | Taffy 0.15 (block, flex, grid, positioned), Parley inline text; tables emulated on grid | floats and vertical writing modes behind optional features |
| Paint | `blitz-paint` onto the `anyrender` trait | backends: Vello (wgpu), Vello CPU, Vello hybrid, Skia |
| Network | `NetProvider` trait; `blitz-net` on reqwest | images, SVG (usvg), web fonts |
| Input, forms, accessibility | `UiEvent`, Parley text editing, AccessKit | |
| JavaScript | `blitz-vibey-script` (9,500 lines, unpublished) on Boa 0.22 | runs Preact; DOM in Rust, JS wrappers hold node ids |

The README says it "can already render many popular no-JS websites (Wikipedia,
(Old) Reddit…)". There are no published memory figures.

Other projects already bind JavaScript to `blitz-dom` the same way (nodes owned
by Rust, JS wrappers holding ids): kiln and blitsen use QuickJS (kiln through
`rquickjs`), oxibrowser uses Boa. kiln reports about 26 MB peak RSS headless
and about 105 MB with an idle window (author's measurement on an M3 Pro, not
verified here).

### JavaScript engine candidates

No engine in the low-memory tier has a JIT; script-heavy sites will be slower
than in Servo. test262 scores are from test262.fyi (22 October 2026).

| Engine | Language / license | test262 | Heap cap / interrupt / bytecode cache | Embedding |
| --- | --- | --- | --- | --- |
| **QuickJS-ng** via `rquickjs` 0.14 | C / MIT | 83.5% | yes / yes / yes | mature Rust crate, one runtime per tab gives a per-tab cap |
| **Boa** 0.22 | Rust / MIT | 95.6% | no / no / no (only loop and recursion limits) | native Rust; Blitz's own JS layer uses it |
| Nova | Rust / MPL-2.0 | 76.9% | not found | young; watch |
| Brimstone | Rust / MIT | not listed | not documented | no embedding API yet; watch |
| Duktape, JerryScript, MicroQuickJS | C | ES5 to partial modern | small fixed heaps | no maintained Rust crate, or too limited for modern sites |

Not chosen: V8 and SpiderMonkey (fast and compatible, but heavy); DOMs written
in JavaScript (linkedom, happy-dom) inside the VM, because they would duplicate
the DOM Blitz already uses for style and layout.

## Design

### Two engines behind one boundary

Wega keeps Servo and adds the Wega engine. The browser chooses per site, shows
which engine a tab uses, and offers *Open in full engine* when a page needs
something the Wega engine does not have yet. Over time the Wega engine becomes
the default and Servo the fallback.

Today the desktop layer calls Servo directly (about 7,300 of 15,300 non-test
lines are Servo glue), and [architecture-decisions.md](architecture-decisions.md)
rejects an engine trait. A new ADR replaces that with one narrow boundary. The
engine-free core (`src/lib.rs` modules, `NavigationPort`, `DownloadPort`,
`PermissionPort`, `Session<T>`, `lifecycle::Activity`) stays as it is.

```rust
/// One per browser window. Servo and the Wega engine each implement it.
pub trait PageEngine {
    type View: PageView;
    fn create_view(&mut self, config: ViewConfig, events: EventSink) -> Self::View;
    fn capabilities(&self) -> Capabilities;
    fn memory_report(&self) -> MemoryReport;
    fn memory_pressure(&mut self, level: Pressure);
}

/// One per tab.
pub trait PageView {
    fn load(&mut self, url: &Url);
    fn reload(&mut self);
    fn stop(&mut self);
    fn traverse(&mut self, delta: i32);
    fn resize(&mut self, size: PhysicalSize, scale: f32);
    fn input(&mut self, event: InputEvent);
    fn paint(&mut self, target: &mut dyn PaintTarget);
    fn set_color_scheme(&mut self, scheme: ColorScheme);
    fn set_zoom(&mut self, factor: f32);
    fn find(&mut self, query: &FindQuery);
    /// Background tabs: drop surfaces and caches, pause timers and scripts.
    fn freeze(&mut self);
    fn thaw(&mut self);
    /// Optional capability; scripting tools (console, reader view) check it.
    fn evaluate_script(&mut self, source: &str, done: ScriptCallback) -> Result<(), Unsupported>;
}

/// What a view reports back to the chrome.
pub enum PageEvent {
    LoadStarted, LoadFinished, TitleChanged(String), UrlChanged(Url),
    FaviconChanged(Image), CursorChanged(Cursor), RepaintNeeded,
    NewWindowRequested(Url), DownloadRequested(Url), Permission(PermissionRequest),
    Dialog(DialogRequest), Unsupported(Feature), Crashed(String),
}
```

`PaintTarget` hides how pixels reach the egui chrome. Servo keeps its shared GL
surface. The Wega engine starts with `anyrender` Vello CPU into a buffer
uploaded as a GL texture, which is simple and works with the existing glow
context, and moves to a GPU path (Skia GL or wgpu interop) once measured.
Devtools, WebDriver and the network panel stay Servo-only at first and are
enabled through `Capabilities`.

### Inside the Wega engine

```text
Wega chrome ──PageView──▶ WegaView
                           ├─ blitz-dom Document (single DOM: style, layout, events)
                           ├─ ScriptHost ── rquickjs runtime (one per tab, capped)
                           ├─ NetProvider ── Wega networking (async, cookies shared)
                           └─ anyrender ── PaintTarget
```

```rust
/// Between the DOM and the JS engine, so the VM can be swapped after the spike.
pub trait ScriptHost {
    fn set_limits(&mut self, limits: ScriptLimits); // heap bytes, GC threshold, max run time
    fn run_script(&mut self, source: ScriptSource, document: &mut DocumentMutator);
    fn dispatch_event(&mut self, target: NodeId, event: &DomEvent) -> DefaultAction;
    fn run_timers_and_microtasks(&mut self, now: Instant);
    fn interrupt(&self);
    fn memory_usage(&self) -> usize;
}
```

- **One DOM.** `blitz-dom` owns all nodes. JS wrappers hold a generational node
  id and are cached per runtime, so identity (`===`) holds and a removed node
  can be collected by both sides.
- **Bindings are generated.** A WebIDL-to-Rust generator targets the
  `ScriptHost` API, as NetSurf does with nsgenbind. NetSurf still lists about
  1,540 unimplemented bindings after a decade of hand work; generation and
  priorities matter more than the VM.
- **Async networking.** `blitz-vibey-script` fetches synchronously; the Wega
  engine uses an async `NetProvider` on Wega's own stack, sharing cookies and
  the download manager.
- **Use, don't fork, where possible.** Depend on published Blitz crates and
  send fixes upstream. Copy `blitz-vibey-script` patterns (MIT/Apache-2.0
  allows it, with attribution) rather than depending on an unpublished crate.

### Memory design

Targets to verify in the spikes, not claims:

| Page type | Servo today | Wega engine target |
| --- | --- | --- |
| Article or documentation page | measure in phase 0 | under 100 MB |
| Interactive app page | about 500 MB observed | under 250 MB |
| Frozen background tab | most of its live memory | under 10 MB |

Techniques built into the engine:

- **Hard per-tab limits:** one `rquickjs` runtime per tab with
  `set_memory_limit`, `set_gc_threshold` and an interrupt handler, so a heavy
  page is stopped and reported instead of taking the container down.
- **Freeze and discard:** background tabs drop paint surfaces, decoded images
  and layout caches; long-idle tabs are serialized and discarded (extending the
  rules in `src/lifecycle.rs`).
- **Images at display size,** decoded lazily and dropped when off-screen.
- **Compact DOM:** measure `blitz-dom`'s per-node cost (estimated at roughly
  1–1.5 KB per laid-out element, mostly Taffy layout caches) and shrink the
  largest parts, such as caches only kept for elements that change.
- **Bytecode caching** of scripts across tabs and restarts.
- **Content blocking** of ads and trackers, often the largest single saving on
  news sites.
- **Allocator purge** after a tab closes, so freed memory goes back to the OS.

Servo-side quick wins come first (phase 0) and help measure: an `about:memory`
page from `servo.create_memory_report` (already used in `src/toolbox.rs`),
trying `servo/multiprocess` so closing a tab returns memory, freeing background
tab surfaces, an option to turn off the JIT, and memory-pressure tab
discarding.

## Phases

| Phase | Work | Exit criteria |
| --- | --- | --- |
| 0. Measure | `about:memory`; Servo quick wins above; a reference-site memory benchmark | numbers for 10 reference pages; known split between JS heap, DOM/style/layout, images and surfaces |
| 1. Boundary | New ADR; `PageEngine`/`PageView` with a Servo adapter; no behaviour change | all native suites pass on the Servo adapter |
| 2. Static spike | `WegaView` on `blitz-dom` without JS, behind a developer flag | reference article pages render; memory and CPU measured against Servo |
| 3. JS spike | `ScriptHost` on `rquickjs` and on Boa (from `blitz-vibey-script`); Preact TodoMVC plus 3 real sites | VM chosen on measured memory, speed and conformance |
| 4. First release | per-site engine choice, engine badge in the address bar, *Open in full engine*, automatic fallback on unsupported features | the Wega engine is the default for an allowlist of sites |
| 5. Grow | WebIDL generator, async fetch, XHR, storage, `MutationObserver`; WPT subset in CI; compatibility dashboard | sites move from the allowlist to default-on as tests pass |

Reference pages for every measurement: a Wikipedia article, Old Reddit, Hacker
News, an MDN page, a docs.rs page, a GitHub repository page, a news site,
Preact TodoMVC, Google search and YouTube (the last two expected to stay on
Servo for a long time).

## Risks

- **Web API breadth is the real cost,** not the VM or layout. Without
  prioritization by real sites, the engine stays a demo.
- **No JIT:** script-heavy sites will be slow; the per-site choice keeps them
  on Servo.
- **Two engines to maintain** until the Wega engine is the default; the
  boundary keeps that cost contained.
- **Security of a new engine:** parsing and scripting untrusted content. One
  process per tab and the existing Docker isolation are the mitigations; a
  sandbox is a later phase.
- **Upstream pace:** Blitz is beta and changes quickly; pin versions and
  upstream fixes.
- **QuickJS-ng's lower test262 score** (83.5% against Bellard's QuickJS at
  98.5%) is not explained yet; the JS spike must check it.

## Licensing

Blitz is `MIT OR Apache-2.0`; Stylo and `stylo_taffy` are MPL-2.0; QuickJS,
QuickJS-ng, `rquickjs` and Boa are MIT. All are compatible with Wega's MPL-2.0.
Copied Blitz code keeps its notices; [licensing.md](licensing.md) and the
third-party notices cover the new dependencies.

## Open decisions

1. The engine's name and whether it lives in this repository or its own.
2. QuickJS-ng (memory controls) or Boa (pure Rust, better conformance): decide
   after the phase 3 spike, or add heap limits to Boa upstream.
3. How the per-site engine choice looks to users: automatic, visible badge,
   manual switch.
4. GPU path for the Wega engine: Skia on the existing GL context, or move the
   chrome to wgpu.
5. Whether to also contribute memory fixes to Servo while it remains the
   fallback.

## Sources

- Servo 0.7 crates: `servo-script`, `servo-script-bindings`, `servo-layout-api`,
  `servo-config` (`prefs.rs` GC and JIT preferences).
- Blitz at commit `1c5265a` (23 October 2026): `blitz-dom/src/document.rs`,
  `node/node.rs`, `layout/mod.rs`, `blitz-paint/src/lib.rs`,
  `blitz-traits/src/net.rs`, `blitz-vibey-script/`.
- Projects on Blitz: <https://github.com/DioxusLabs/blitz/issues/613>, kiln
  <https://github.com/marshallshelly/kiln>.
- JS conformance: <https://test262.fyi>; `rquickjs` limits in
  `core/src/runtime/base.rs`; Boa limits in `core/engine/src/vm/runtime_limits.rs`.
- NetSurf JavaScript bindings: `docs/jsbinding.md`, `docs/UnimplementedJavascript.md`.
