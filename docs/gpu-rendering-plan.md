# Plan: optional GPU rendering in the Docker image

Status: proposed, not implemented. This needs code changes in the image; it
cannot be switched on with configuration alone.

## Goal

When the host passes a GPU render node (`/dev/dri/renderD*`) into the container,
Wega renders on that GPU. Without one, the image behaves exactly as today
(Mesa llvmpipe on the CPU). Operators opt in by adding the device; nothing
changes for existing deployments.

The main gain is CPU: with llvmpipe, rasterizing and compositing a busy page uses
all CPUs the container is given (`WEGA_RENDER_THREADS`, default 2). It is not a
memory fix. On integrated GPUs (for example AMD APUs and Intel iGPUs) graphics
memory is carved from system RAM, so the total RAM a page costs stays similar.

## Why it does not work today

| Finding | Effect |
| --- | --- |
| The browser draws into **Xvfb** (`docker/supervisord.conf`), a virtual X server with no GPU path | Mesa can only use llvmpipe; `docker/start-browser.sh` forces `GALLIUM_DRIVER=llvmpipe` |
| Servo's Linux backend (surfman 0.14) uses **EGL**, not GLX; winit supports X11 and Wayland | Both an EGL interposer and a native Wayland session are possible |
| `gosu` sets supplementary groups from `/etc/group` and the root filesystem is read-only | A `group_add` render group from Compose is dropped before the browser starts, so `/dev/dri/renderD128` would be unreadable |
| **Docker Swarm services cannot be given devices.** A `/dev/dri` bind mount is visible but opening it fails with `Operation not permitted` (device cgroup) | Swarm deployments must run Wega as a plain Compose service with `devices:`, as is already done for hardware-transcoding media servers |

A real-world check: a `linuxserver/firefox` Swarm service with `/dev/dri`
bind-mounted had no process holding the render node and no Mesa hardware driver
loaded. It also renders in software.

## Options

### A. Keep Xvfb, render through VirtualGL's EGL interposer (recommended first)

VirtualGL 3.1+ interposes EGL/X11 applications: GL runs on the render node and
each finished frame is read back into the Xvfb window. x11vnc, noVNC, openbox,
clipboard, resize, uploads and all Docker tests stay as they are.

- Change: install VirtualGL (not packaged in Debian; pin an upstream `.deb` by
  version and SHA-256), start the browser with
  `vglrun -d /dev/dri/renderD128` when GPU mode is active.
- Cost: one frame readback per paint (about 8 MB at 1920x1080), which is
  cheap next to llvmpipe rasterizing the frame.
- Risk: surfman's `eglGetPlatformDisplay` / surface use must work under the
  interposer. That is the first thing the spike proves or rules out.

### B. Replace Xvfb with a headless Wayland compositor and wayvnc

labwc (wlroots, `WLR_BACKENDS=headless`, `WLR_RENDERER=gles2`,
`WLR_RENDER_DRM_DEVICE=/dev/dri/renderD128`) hosts Wega as a native Wayland
client; wayvnc serves VNC on the existing Unix socket, so websockify, nginx and
the viewer stay unchanged.

- No interposer and no X server; the same stack also runs in software
  (wlroots pixman renderer + llvmpipe), so one display stack could serve both
  modes.
- Larger change: replaces three supervisor programs and the openbox rules
  (undecorated, maximized, detached tool windows), and needs new answers for
  remote resize (wlr-randr), clipboard (wayvnc) and the Docker test scripts
  that expect x11vnc. Wega's own Wayland path also needs native testing.

Use B if A fails the spike, or later if the readback cost or X11 maintenance
becomes the bottleneck.

### Not chosen

- **Xorg with a real driver (modesetting/amdgpu, glamor):** needs a KMS
  device with a connector and is fragile on headless hosts.
- **Separate GPU image:** doubles the images to build and test. One image with
  runtime detection is enough.

## Design (option A)

1. **Switch** `WEGA_GPU=auto|off|/dev/dri/renderDNNN` (default `auto`).
   `auto` uses the first render node the browser user can open; otherwise
   llvmpipe. An explicit path that cannot be opened fails startup with a clear
   message rather than silently falling back.
2. **Groups.** The entrypoint (root) reads the render node's GID and starts the
   browser program with `setpriv --reuid browser --regid browser
   --groups wega-ipc,<gid>` instead of `gosu`, because `/etc/group` cannot be
   edited on the read-only root. No new capabilities are required.
3. **Launch.** `start-browser.sh` exports llvmpipe settings only in software
   mode and wraps Wega with `vglrun -d <node>` in GPU mode.
4. **Visibility.** Wega logs `GL_VENDOR`/`GL_RENDERER` once after creating its
   context (small Rust change in `browser.rs`), and the entrypoint logs the
   chosen mode, so operators can tell which path is active from `docker logs`.
5. **Docs.** `docs/docker.md` gains the switch, a Compose example
   (`devices: [/dev/dri/renderD128]`, `group_add` not needed), the Swarm
   limitation, and the security note below.

### Security

Passing a render node exposes the host's GPU kernel driver to a process that
renders untrusted web content. This is a real increase in attack surface, which
is why GPU mode is opt-in and documented as a trade-off. Only the render node is
needed, not `/dev/dri/card*` (modesetting).

## Steps

1. **Spike (about a day):** on a host with an AMD or Intel GPU, run the current
   image under plain Compose with the render node, VirtualGL installed and
   Wega under `vglrun`; confirm `GL_RENDERER` names the GPU and pages draw.
   If it fails, record why and switch to option B.
2. Implement the switch, group handling and launch wrapper (entrypoint,
   `start-browser.sh`, `supervisord.conf`, Dockerfile).
3. Add the renderer log line in Wega.
4. Tests: shell contracts for `off`, `auto` without a device, an explicit
   unreadable path (startup fails) and GID handling with a fake device node; the
   existing Docker CI keeps covering software mode, since runners have no GPU.
5. Measure on the GPU host with the same pages in both modes: CPU %, frame
   rate during scrolling, and container memory. Record the numbers in this
   document.
6. Document and release.

## Open questions

- Whether surfman works under VirtualGL's EGL interposer (the spike decides
  between A and B).
- Whether video decoding through GStreamer should also use the GPU (VA-API).
  That is a separate change; this plan covers rendering only.
