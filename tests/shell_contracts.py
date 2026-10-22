#!/usr/bin/env python3
"""Linux-container contracts for executable shell adapters, collected by kcov.

Run in the disposable test-tools image, never on a host: entrypoint tests write
/config and /run. Packaging uses a private checkout and explicit fake compiler /
media-packager ports; actual cp/chmod/tar and generated launchers are exercised.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not Path('/.dockerenv').exists() or os.geteuid() != 0:
        raise SystemExit('Run only as root in a disposable Docker test container')
    source, output = args.source.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    for name in ['lcov.info', 'measurement.json']:
        (output / name).unlink(missing_ok=True)
    raw = output / 'raw'
    raw.mkdir(exist_ok=True)
    collection = Path(tempfile.mkdtemp(prefix='run-', dir=raw))
    with tempfile.TemporaryDirectory(prefix='lotse-shell-') as temporary:
        work = Path(temporary)
        checkout = work / 'checkout'
        for name in ['docker', 'scripts', 'assets']:
            shutil.copytree(source / name, checkout / name)
        shutil.copy2(source / 'LICENSE', checkout / 'LICENSE')
        fake = work / 'bin'
        fake.mkdir()
        calls = work / 'calls.jsonl'
        def program(name, body):
            path = fake / name
            path.write_text('#!/usr/bin/python3\nimport os,sys,json\nfrom pathlib import Path\n' + body)
            path.chmod(0o755)
        log = f"with open({str(calls)!r},'a') as f: f.write(json.dumps([Path(sys.argv[0]).name,sys.argv[1:]])+'\\n')\n"
        program('cargo', log)
        program('python3', log)
        program('uname', "print('Darwin')\n")
        program('supervisord', log + "assert not any(k in os.environ for k in ['BROWSER_USER','BROWSER_PASSWORD','BROWSER_PASSWORD_FILE'])\n")
        program('supervisorctl', "print('\\n'.join(f'worker{i} RUNNING pid 1' for i in range(6)))\n")
        program('curl', "print('401' if '/auth/account' in sys.argv[-1] else '')\n")
        program('xdpyinfo', "p=Path('/tmp/lotse-display-attempts');n=int(p.read_text()) if p.exists() else 0;p.write_text(str(n+1));sys.exit(0 if n else 1)\n")
        program('servo-browser', log)
        base_env = os.environ.copy()
        base_env['PATH'] = str(fake) + ':' + base_env['PATH']
        runs = []
        def run(name, arguments=(), *, env=None, success=True):
            directory = collection / f'case-{len(runs):02}'
            command = ['kcov', f'--include-path={checkout}', str(directory), str(checkout / name), *arguments]
            result = subprocess.run(command, env=env or base_env, capture_output=True, text=True, timeout=90)
            assert (result.returncode == 0) == success, (name, arguments, result.returncode, result.stderr[-2000:])
            runs.append(directory)
            return result
        # Linux packaging: real filesystem/archive operations, isolated fake binary.
        run('scripts/package-linux.sh', [str(work / 'missing')], success=False)
        binary = checkout / 'target/release/servo-browser'
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/sh\nprintf "launcher:%s\\n" "$1"\n')
        binary.chmod(0o755)
        run('scripts/package-linux.sh')
        package = checkout / 'dist/lotse-linux-x86_64'
        assert (package / 'share/applications/lotse.desktop').read_text().startswith('[Desktop Entry]')
        assert subprocess.check_output([str(package / 'bin/lotse'), 'literal argument'], text=True).strip() == 'launcher:literal argument'
        shutil.move(checkout / 'LICENSE', work / 'LICENSE')
        shutil.move(checkout / 'assets/icons/browser-icon-256.png', work / 'icon.png')
        run('scripts/package-linux.sh', [str(binary)])
        shutil.move(work / 'LICENSE', checkout / 'LICENSE')
        # macOS packager contracts substitute only external compiler/media ports.
        linux_env = dict(base_env, PATH='/usr/bin:/bin')
        run('scripts/package-macos.sh', env=linux_env, success=False)
        run('scripts/package-macos.sh', ['--invalid'], success=False)
        run('scripts/package-macos.sh', ['--no-media'])
        debug = checkout / 'target/debug/servo-browser'
        debug.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(binary, debug)
        run('scripts/package-macos.sh', ['--debug', '--media'])
        assert (checkout / 'target/Lotse.app/Contents/MacOS/servo-browser').read_bytes() == debug.read_bytes()
        assert 'org.medimesh.ServoBrowser' in (checkout / 'target/Lotse.app/Contents/Info.plist').read_text()
        commands = [json.loads(line) for line in calls.read_text().splitlines()]
        assert ['cargo', ['build', '--locked', '--release', '--no-default-features', '--features', 'brotli-streams']] in commands
        assert ['cargo', ['build', '--locked', '--features', 'media']] in commands
        assert any(name == 'python3' and arguments[0] == 'scripts/bundle-macos-media.py' for name, arguments in commands)
        # The gateway bootstrap is already tested against real encryption/storage.
        # Here its process boundary is recorded, while shell credential scrubbing,
        # directory modes and real self-signed TLS generation are checked.
        env = dict(base_env, BROWSER_RESOLUTION='1280x800', BROWSER_USER='fixture-user', BROWSER_PASSWORD='fixture-secret', BROWSER_PASSWORD_FILE='/tmp/fixture-password')
        Path('/config/auth').mkdir(parents=True, exist_ok=True)
        run('docker/entrypoint.sh', env=env)
        assert (Path('/config/tls/server.key').stat().st_mode & 0o777) == 0o600
        assert (Path('/run/servo').stat().st_mode & 0o777) == 0o700
        run('docker/entrypoint.sh', ['/bin/sh', '-c', 'test -z "${BROWSER_USER+x}${BROWSER_PASSWORD+x}${BROWSER_PASSWORD_FILE+x}"'], env=env)
        for resolution in ['', 'bad', '800x600x2', 'x600', '800x', '799x600', '800x599', '3841x600', '800x2161']:
            run('docker/entrypoint.sh', env=dict(env, BROWSER_RESOLUTION=resolution), success=False)
        run('docker/entrypoint.sh', ['healthcheck'], env=env)
        Path('/config/browser').mkdir(parents=True, exist_ok=True)
        profile = Path('/config/browser/browser.json')
        profile.unlink(missing_ok=True)
        env = dict(base_env, DISPLAY=':99', BROWSER_START_URL='https://example.com/with literal space')
        run('docker/start-browser.sh', env=env)
        profile.write_text('{}')
        run('docker/start-browser.sh', env=env)
        commands = [json.loads(line) for line in calls.read_text().splitlines()]
        assert ['servo-browser', ['--profile-dir', '/config/browser', '--', env['BROWSER_START_URL']]] in commands
        assert ['servo-browser', ['--profile-dir', '/config/browser']] in commands
        # Merge exact collector counters, never infer hits from source text.
        merged = collection / 'merged'
        subprocess.run(['kcov', '--merge', str(merged), *map(str, runs)], check=True, capture_output=True, timeout=60)
        samples = {}
        for report in merged.rglob('cobertura.xml'):
            for item in ET.parse(report).findall('.//class'):
                path = Path(item.attrib['filename'])
                if not path.is_absolute():
                    path = checkout / path
                try:
                    name = path.relative_to(checkout).as_posix()
                except ValueError:
                    continue
                if name not in ['docker/entrypoint.sh', 'docker/start-browser.sh', 'scripts/package-linux.sh', 'scripts/package-macos.sh']:
                    continue
                counters = samples.setdefault(name, {})
                for line in item.findall('./lines/line'):
                    number, hits = int(line.attrib['number']), int(line.attrib['hits'])
                    counters[number] = max(counters.get(number, 0), hits)
        assert len(samples) == 4, f'Incomplete shell collector output: {list(samples)}'
        lcov = []
        hashes = {}
        for name, counters in sorted(samples.items()):
            hashes[name] = hashlib.sha256((source / name).read_bytes()).hexdigest()
            lcov += [f'SF:{name}', *[f'DA:{line},{hits}' for line, hits in sorted(counters.items())], f'LF:{len(counters)}', f'LH:{sum(hit > 0 for hit in counters.values())}', 'end_of_record']
            print(f'{name}: {sum(hit > 0 for hit in counters.values())}/{len(counters)} lines; missing {[line for line, hit in counters.items() if hit == 0]}')
        (output / 'lcov.info').write_text('\n'.join(lcov) + '\n')
        (output / 'measurement.json').write_text(json.dumps({'source_hashes': hashes, 'tests_passed': True, 'collector': subprocess.check_output(['kcov', '--version'], text=True).strip(), 'scope': 'shell process-boundary contracts in disposable Linux container'}, indent=2) + '\n')
        print('Shell process-boundary contracts passed')

if __name__ == '__main__':
    main()
