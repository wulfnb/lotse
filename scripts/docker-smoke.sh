#!/bin/sh
# Requires a running Compose service. Uses an isolated browser profile/display.
set -eu
cd "$(dirname "$0")/.."
docker compose exec -T wega sh -eu -c '
    ldd /usr/local/bin/wega | tee /tmp/wega-ldd.txt
    ! grep -q "not found" /tmp/wega-ldd.txt
    curl -fkSs https://127.0.0.1:7443/healthz
    python3 -c '\''import json, os, pathlib, runpy, ssl, urllib.request
helpers = runpy.run_path("/opt/wega/auth-service.py")
decode = helpers["decode"]
password = pathlib.Path(os.environ["BROWSER_PASSWORD_FILE"]).read_text().rstrip("\r\n") if os.environ.get("BROWSER_PASSWORD_FILE") else os.environ["BROWSER_PASSWORD"]
body = json.dumps({"username": decode(os.environ.get("BROWSER_USER", "servo")), "password": decode(password)}).encode()
request = urllib.request.Request("https://127.0.0.1:7443/auth/login", data=body, headers={"Content-Type": "application/json", "Origin": "https://127.0.0.1:7443"})
context = ssl._create_unverified_context()
with urllib.request.urlopen(request, context=context) as response:
    cookie = response.headers["Set-Cookie"].split(";", 1)[0]
request = urllib.request.Request("https://127.0.0.1:7443/novnc/core/rfb.js", headers={"Cookie": cookie})
with urllib.request.urlopen(request, context=context) as response:
    assert response.status == 200
'\''
    supervisorctl -c /etc/wega-supervisord.conf status
'
docker compose exec -T wega mkdir -p /tmp/wega-docker-fixtures
# The image filesystem is read-only (docker cp cannot write to it); stream into /tmp (tmpfs).
COPYFILE_DISABLE=1 tar -C tests/fixtures -cf - . | docker compose exec -T wega tar -C /tmp/wega-docker-fixtures --no-same-owner -xf -
docker compose exec -T --user browser wega sh -eu -c '
    test_dir=$(mktemp -d /tmp/wega-smoke.XXXXXX)
    Xvfb :98 -screen 0 1440x900x24 -nolisten tcp -ac >/tmp/wega-smoke-display.log 2>&1 &
    display_pid=$!
    trap "kill $display_pid 2>/dev/null || true" EXIT INT TERM
    export DISPLAY=:98
    attempts=0
    until xdpyinfo >/dev/null 2>&1; do
        attempts=$((attempts + 1))
        test "$attempts" -le 50
        sleep 0.2
    done
    timeout 150 wega --smoke-test --profile-dir "$test_dir" file:///tmp/wega-docker-fixtures/interaction.html
    test -s "$test_dir/smoke-page.png"
    printf "Linux smoke test profile: %s\n" "$test_dir"
'
