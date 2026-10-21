#!/bin/sh
# Uses an isolated temporary account and local port 6084; leaves the real account unchanged.
set -eu
cd "$(dirname "$0")/.."
docker compose cp tests/web_auth_test.py servo-browser:/tmp/servo-web-auth-test.py
docker compose exec -T --user browser servo-browser python3 /tmp/servo-web-auth-test.py
