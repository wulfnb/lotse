#!/usr/bin/env python3
"""Measure first-party Rust from unit tests AND real engine suites.

RUSTC_WORKSPACE_WRAPPER instruments Lotse only; third-party crates keep their
normal flags. LLVM_PROFILE_FILE is inherited by browsers started by Python tests.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import shutil
from coverage_sources import fingerprint

ROOT = Path(__file__).resolve().parents[1]
# These are test drivers, not application implementations. An exact list prevents
# names like inspector.rs from accidentally disappearing from the denominator.
HARNESS = ['smoke.rs', 'download_test.rs', 'theme_test.rs', 'capabilities_test.rs', 'toolbox_test.rs']


def collect_artifacts(path):
    executables = []
    for line in path.read_text().splitlines():
        try:
            record = json.loads(line)
        except ValueError:
            continue  # libtest emits ordinary text after Cargo's JSON messages.
        if record.get('reason') == 'compiler-artifact' and record.get('executable'):
            executables.append(record['executable'])
    return executables


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument('--report-only', action='store_true')
    parser.add_argument('--core-only', action='store_true')
    parser.add_argument('--output', default='target/coverage/desktop')
    parser.add_argument('--jobs', default='4')
    args = parser.parse_args(argv)
    output = (ROOT / args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    raw = output / 'raw'
    raw.mkdir(exist_ok=True)
    branch = os.environ.get('LOTSE_COVERAGE_BRANCH') == '1'
    env = os.environ.copy()
    env.update(RUSTC_WORKSPACE_WRAPPER=str(ROOT / 'scripts/coverage-rustc.sh'), LLVM_PROFILE_FILE=str(raw / ('%m-%p%c.profraw' if sys.platform == 'darwin' else '%m-%p.profraw')))
    status = 0
    source_fingerprint = fingerprint(ROOT)
    manifest = output / 'measurement.json'
    if not args.report_only:
        for path in raw.glob('*.profraw'):
            path.unlink()
        cargo_args = ['--locked', '-j', args.jobs, '--message-format=json']
        feature_args = ['--no-default-features', '--lib'] if args.core_only else ['--features', 'test-harness']
        with (output / 'test-artifacts.jsonl').open('w') as log:
            result = subprocess.run(['cargo', 'test', *cargo_args, *feature_args], cwd=ROOT, env=env, stdout=log)
        status |= bool(result.returncode)
        if result.returncode:
            print((output / 'test-artifacts.jsonl').read_text(), file=sys.stderr)
        objects = collect_artifacts(output / 'test-artifacts.jsonl')
        if not args.core_only:
            with (output / 'build-artifacts.jsonl').open('w') as log:
                result = subprocess.run(['cargo', 'build', *cargo_args, '--features', 'test-harness'], cwd=ROOT, env=env, stdout=log)
            status |= bool(result.returncode)
            binaries = collect_artifacts(output / 'build-artifacts.jsonl')
            frozen = []
            for binary in binaries:
                destination = output / 'objects' / Path(binary).name
                destination.parent.mkdir(exist_ok=True)
                temporary = destination.with_suffix('.new')
                shutil.copy2(binary, temporary)
                temporary.replace(destination)
                frozen.append(str(destination))
            binaries = frozen
            objects += binaries
            if binaries:
                native_base = output / 'native-tests'
                native_base.mkdir(exist_ok=True)
                native_output = Path(tempfile.mkdtemp(prefix='run-', dir=native_base))
                result = subprocess.run([sys.executable, str(ROOT / 'scripts/native-tests.py'), '--binary', binaries[-1], '--output', str(native_output)], cwd=ROOT, env=env)
                status |= bool(result.returncode)
                profile_errors = [str(path) for path in native_output.rglob('browser.log') if 'LLVM Profile Error:' in path.read_text()]
                if profile_errors:
                    print('Invalid profiler capture: ' + ', '.join(profile_errors), file=sys.stderr)
                    status = 1
        manifest.write_text(json.dumps({'objects': sorted(set(objects)), 'core_only': args.core_only, 'branches': branch, 'platform': subprocess.check_output(['rustc', '-vV'], text=True), 'source_root': str(ROOT), 'tests_passed': not status and fingerprint(ROOT) == source_fingerprint, 'source_fingerprint': source_fingerprint}, indent=2) + '\n')
    measurement = json.loads(manifest.read_text())
    if measurement.get('source_fingerprint') != fingerprint(ROOT):
        sys.exit('Rust sources changed or are unverifiable; rerun measurement before exporting coverage')
    objects = measurement['objects']
    profiles = list(raw.glob('*.profraw'))
    if not objects or not profiles:
        sys.exit('No instrumented objects or profiles; refusing an empty coverage report')
    sysroot = Path(subprocess.check_output(['rustc', '--print', 'sysroot'], text=True).strip())
    host = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host:'))
    llvm = sysroot / 'lib/rustlib' / host / 'bin'
    merged = output / 'coverage.profdata'
    subprocess.run([str(llvm / 'llvm-profdata'), 'merge', '-sparse', *map(str, profiles), '-o', str(merged)], check=True)
    ignore = r'(/vendor/|/tests/|_tests\.rs$|/src/(' + '|'.join(re.escape(name) for name in HARNESS) + r')$)'
    common = [objects[0], *[arg for obj in objects[1:] for arg in ['--object', obj]], '--instr-profile', str(merged), '--ignore-filename-regex', ignore]
    for kind, arguments, destination in [('export', [], 'coverage.json'), ('export', ['--format=lcov'], 'lcov.info'), ('report', [], 'summary.txt')]:
        with (output / destination).open('w') as stream:
            subprocess.run([str(llvm / 'llvm-cov'), kind, *common, *arguments], check=True, stdout=stream)
    subprocess.run([str(llvm / 'llvm-cov'), 'show', *common, '--format=html', '--output-dir', str(output / 'html')], check=True, stdout=subprocess.DEVNULL)
    print((output / 'summary.txt').read_text())
    return int(status or not measurement['tests_passed'])


if __name__ == '__main__':
    sys.exit(main())
