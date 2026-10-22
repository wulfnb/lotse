#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
mkdir -p target/coverage/javascript/raw
export NODE_V8_COVERAGE="$PWD/target/coverage/javascript/raw"
node --experimental-vm-modules --experimental-test-coverage --test \
    --test-coverage-include='**/docker/viewer/*.js' \
    --test-coverage-include='**/src/*.js' --test-coverage-include='**/site/*.js' --test-coverage-lines=100 \
    --test-reporter=lcov tests/viewer_login_tests.mjs tests/viewer_tests.mjs tests/page_actions_tests.mjs tests/probe_scripts_tests.mjs tests/site_tests.mjs \
    > target/coverage/javascript/lcov.info
# Unmeasured embedded page scripts remain visible in the project inventory;
# passing the viewer component gate is not a claim about those scripts.
