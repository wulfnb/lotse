# epaint 0.37.0 compatibility patch

`epaint/` is the published epaint 0.37.0 crate from crates.io, with its original
MIT/Apache license files. It is used through `[patch.crates-io]`.

Only `src/text/font_face.rs` is changed: both glyph rasterization contexts use
`RenderContext::new_with` with `num_threads: 0` instead of `RenderContext::new`.

Servo enables vello_cpu 0.2's `multithreading` feature. Cargo unifies that feature
with epaint's dependency, changing the default render context to multithreaded.
epaint's glyph path assumes single-threaded behavior and doesn't flush before
rendering, causing `attempted to rasterize before flushing` on the first glyph.
Explicit single-threaded glyph contexts preserve epaint's intended behavior and
avoid creating a worker pool for every tiny glyph. Servo's canvas renderer keeps
its own multithreaded contexts. The browser includes a font-rasterization regression
test under the combined dependency graph.

Remove this patch when the upstream epaint glyph context explicitly sets its
threading mode or correctly handles the multithreaded API. No global Cargo cache
or installed dependency sources are modified.

## Servo 0.7.0 native developer-tool hook

`servo/` is the published MPL-2.0 Servo 0.7.0 crate. One change
is in `servo.rs`: `ServoBuilder::devtools_event_sender` accepts an optional native
consumer of the existing `DevtoolsControlMsg` stream. When supplied, it replaces
the TCP developer server. The browser enables the engine's developer machinery
and feeds events to its native toolbox without exposing a listening port.
The parser, networking, rendering, script engine and protocol types are unchanged.
Remove this patch when upstream exposes an equivalent embedder hook.

## GStreamer SDK discovery

On macOS, `servo.rs` first loads its fixed plugin list from the app executable’s
`lib` directory. For unbundled development builds it also accepts
`SERVO_GSTREAMER_PLUGIN_DIR`, then the standard Apple Silicon/Intel Homebrew
GStreamer plugin locations. The packaged media build copies the plugin list and
native dependency closure into the app; no installed SDK libraries are modified.
