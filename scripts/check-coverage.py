#!/usr/bin/env python3
"""Enforce exact component line targets; rounded percentages never certify 100%."""
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def enforce(baseline, rust, python):
    lines = rust['data'][0]['totals']['lines']
    gateway = next(record['summary'] for name, record in python['files'].items() if name.replace('\\', '/').endswith('docker/auth-service.py'))
    counters = {
        'rust_core_line_percent': (lines['covered'], lines['count']),
        'python_gateway_line_percent': (gateway['covered_lines'], gateway['num_statements']),
    }
    failed = False
    for name, (covered, total) in counters.items():
        minimum = baseline[name]
        if total <= 0 or not 0 <= covered <= total or not 0 <= minimum <= 100:
            raise ValueError('Empty or invalid coverage counters')
        actual = covered * 100 / total
        passed = covered == total if minimum == 100 else actual >= minimum
        print(f'{name}: {covered}/{total} = {actual:.4f}% (required {minimum:.2f}%)')
        failed |= not passed
    return not failed


def main(root=ROOT):
    try:
        baseline = json.loads((root / 'coverage-baseline.json').read_text())
        rust = json.loads((root / 'target/coverage/rust/coverage.json').read_text())
        python = json.loads((root / 'target/coverage/python/coverage.json').read_text())
        passed = enforce(baseline, rust, python)
    except (OSError, ValueError, KeyError, IndexError, StopIteration, TypeError) as error:
        print(f'Coverage reports missing or invalid: {error}', file=sys.stderr)
        return 1
    if not passed:
        print('Coverage target is unmet. Add tests; do not hide uncovered code or lower the target.', file=sys.stderr)
    return int(not passed)


if __name__ == '__main__':
    sys.exit(main())
