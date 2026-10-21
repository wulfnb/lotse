#!/bin/sh
set -eu
until xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; do sleep 0.2; done
set -- servo-browser --profile-dir /config/browser
if [ ! -s /config/browser/browser.json ]; then set -- "$@" -- "$BROWSER_START_URL"; fi
exec "$@"
