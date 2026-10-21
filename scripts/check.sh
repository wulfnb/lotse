#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo fmt --check
cargo clippy --locked --no-default-features --all-targets -- -D warnings
cargo test --locked --no-default-features --lib
if [ -z "${PYTHON:-}" ]; then
    if [ -x ".venv/bin/python" ]; then
        PYTHON=".venv/bin/python"
    else
        PYTHON="python3"
    fi
fi

if "$PYTHON" -c "import cryptography" >/dev/null 2>&1; then
    "$PYTHON" -m unittest discover -s tests -p 'test_*.py' -v
else
    echo "Notice: Python 'cryptography' module not found in $PYTHON environment." >&2
    echo "Notice: Install dependencies via 'pip install -r requirements-dev.txt' or use a virtualenv to run full gateway auth tests." >&2
    "$PYTHON" -m unittest discover -s tests -p 'test_coverage_*.py' -v
    "$PYTHON" -m unittest discover -s tests -p 'test_release_*.py' -v
fi
# Desktop checks are opt-in locally; CI runs them on a prepared macOS runner.
if [ "${CHECK_DESKTOP:-0}" = 1 ]; then
    cargo clippy --locked --all-targets -- -D warnings
    cargo clippy --locked --all-targets --features test-harness -- -D warnings
    cargo test --locked --features test-harness
fi
