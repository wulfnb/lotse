#!/bin/sh
# Run the real HTTP developer-tools suite without touching the user's session.
set -eu
cd "$(dirname "$0")/.."
docker compose exec -T servo-browser mkdir -p /tmp/servo-toolbox-suite
docker compose cp tests/. servo-browser:/tmp/servo-toolbox-suite
docker compose exec -T --user browser servo-browser sh -eu -c '
    test_dir=$(mktemp -d /tmp/servo-toolbox.XXXXXX)
    Xvfb :97 -screen 0 1600x1000x24 -nolisten tcp -ac >"$test_dir/display.log" 2>&1 &
    display_pid=$!
    python3 /tmp/servo-toolbox-suite/network_server.py 8767 >"$test_dir/http.log" 2>&1 &
    server_pid=$!
    trap "kill $display_pid $server_pid 2>/dev/null || true" EXIT INT TERM
    export DISPLAY=:97
    attempts=0
    until xdpyinfo >/dev/null 2>&1 && curl -fsS http://127.0.0.1:8767/ >/dev/null 2>&1; do
        attempts=$((attempts + 1))
        test "$attempts" -le 50
        sleep 0.2
    done
    timeout 260 servo-browser --toolbox-test --profile-dir "$test_dir"
    printf "Linux developer-tools test profile: %s\n" "$test_dir"
'
