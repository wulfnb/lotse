#!/bin/sh
# Cargo invokes this only for workspace crates. Dependencies stay uninstrumented.
# Branch coverage requires a nightly toolchain; stable runs must never pretend
# that a missing branch map means every branch was covered.
# Darwin continuous profiles map counters directly, including after SIGTERM.
# Align every mapped profile section to the Apple Silicon page size (also valid
# on Intel). Without this, LLVM silently leaves killed-driver counters at zero.
if [ "$(uname -s)" = Darwin ]; then
    set -- "$@" -C link-arg=-Wl,-sectalign,__DATA,__llvm_prf_cnts,0x4000 \
        -C link-arg=-Wl,-sectalign,__DATA,__llvm_prf_data,0x4000 \
        -C link-arg=-Wl,-sectalign,__DATA,__llvm_prf_bits,0x4000
fi
if [ "${WEGA_COVERAGE_BRANCH:-0}" = 1 ]; then
    exec "$@" -C instrument-coverage -Z coverage-options=branch
fi
exec "$@" -C instrument-coverage
