#!/bin/sh
# Core and gateway coverage deliberately exclude Servo, native UI and test drivers.
set -eu
cd "$(dirname "$0")/.."
python_bin=${PYTHON:-python3}
mkdir -p target/coverage/rust target/coverage/python
cargo llvm-cov clean --workspace
cargo llvm-cov --locked --no-default-features --lib --json --output-path target/coverage/rust/coverage.json -j "${BUILD_JOBS:-4}"
cargo llvm-cov report --lcov --output-path target/coverage/rust/lcov.info
cargo llvm-cov report --html --output-dir target/coverage/rust
"$python_bin" -m coverage erase
"$python_bin" -m coverage run -m unittest discover -s tests -p 'test_*.py' -v
"$python_bin" -m coverage html
"$python_bin" -m coverage xml
"$python_bin" -m coverage json
"$python_bin" scripts/check-coverage.py
