#!/bin/sh
set -eu
until xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; do sleep 0.2; done
# Xvfb has no GPU path: Mesa renders on the CPU with llvmpipe. Bound its
# rasterizer threads so one busy page cannot occupy every host core.
export LP_NUM_THREADS="${WEGA_RENDER_THREADS:-2}" GALLIUM_DRIVER="${WEGA_GALLIUM_DRIVER:-llvmpipe}"
set -- wega --profile-dir /config/browser
if [ ! -s /config/browser/browser.json ]; then set -- "$@" -- "$BROWSER_START_URL"; fi
exec "$@"
