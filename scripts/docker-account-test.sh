#!/bin/sh
# Uses an isolated temporary account and private Unix sockets; leaves the real account unchanged.
set -eu
cd "$(dirname "$0")/.."
# The image filesystem is read-only (docker cp cannot write to it); stream into /tmp (tmpfs).
docker compose exec -T wega sh -c 'cat > /tmp/wega-web-auth-test.py' < tests/web_auth_test.py
docker compose exec -T --user browser wega python3 /tmp/wega-web-auth-test.py
