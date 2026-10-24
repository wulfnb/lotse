# Wega engine worker

This independent workspace builds `wega-engine`, a private, headless page worker
using Blitz 0.3.0-beta.4, QuickJS-ng through rquickjs 0.14, and Vello CPU. Wega
owns the orchestration, DOM bindings, limits, networking and pipe integration.
Upstream crates remain attributed and unmodified.

Servo's pinned SpiderMonkey requires ICU 2.1, whereas Blitz beta.4 requires ICU
2.3. They cannot resolve together in one Cargo graph. Each workspace therefore
has its own lockfile and target directory. No Servo or SpiderMonkey engine is
linked into this worker. `src/engine_protocol.rs` is shared by source with the
browser; protocol version and payload bounds are validated on both sides.

```sh
python3 scripts/build-wega-engine.py --debug
cargo run --locked -- --engine wega https://example.org/
cargo test --locked --manifest-path engines/Cargo.toml
```

The native bottom-bar selector reloads only the selected tab. Open-tab engine
choices survive profile save/restore. A worker owns one DOM and one VM; closing
or discarding that tab terminates it. Background tabs suspend scripting and drop
paint surfaces. Parent IPC retains only the latest frame and never waits for a
script on the UI thread. Servo initializes lazily when the first Servo tab is
used; once initialized it remains process-scoped.

The worker has a 32 MiB JS heap cap, execution interruption, a bounded microtask
batch, HTML/resource/node/pixel bounds and a four-thread network queue. These
limits do not constitute a total RSS limit or an OS sandbox. Rendering is capped
at 30 frames per second. Idle workers block waiting for input or resource/timer
deadlines. Public pages may still require much more browser API support.

This is an experimental first integration: HTML/CSS, external assets, basic
classic scripts, DOM queries/text/attributes/node operations, click listeners,
simple timers, navigation, scrolling, zoom and capture. Forms, modules, fetch/XHR,
storage/cookies/sign-in, media, content accessibility and developer tools require
Servo. Unsupported script APIs report an error; switching does not transfer
cookies, live DOM or unsaved form state. There is no automatic fallback or
per-site preference yet. Use Servo for everyday browsing until the documented
compatibility and memory benchmarks justify changing the default.
