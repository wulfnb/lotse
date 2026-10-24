#!/usr/bin/env python3
"""Check real Launch Services delivery with a unique disposable app/profile.

Never changes the default browser. Removes only this test app's registration.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unicodedata
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.parse
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', default=str(ROOT / 'target/debug/wega'))
    parser.add_argument('--output', default=str(ROOT / 'target/macos-launch-tests'))
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('Launch Services testing requires macOS')
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix='launch-', dir=output))
    app = work / 'Wega Launch Test.app'
    executable = app / 'Contents/MacOS/wega'
    executable.parent.mkdir(parents=True)
    shutil.copy2(Path(args.binary).resolve(), executable)
    xml = (ROOT / 'scripts/package-macos.sh').read_text().split("<<'PLIST'\n", 1)[1].split('\nPLIST', 1)[0]
    info = plistlib.loads(xml.encode())
    info['CFBundleIdentifier'] = 'io.github.wulfnb.wega.launchtest.' + uuid.uuid4().hex
    info['CFBundleName'] = info['CFBundleDisplayName'] = 'Wega Launch Test'
    for destination in [app / 'Contents/Info.plist', work / 'Info.plist']:
        destination.write_bytes(plistlib.dumps(info))
    profile = work / 'profile'
    profile.mkdir()
    log = work / 'browser.log'
    log.touch(mode=0o600)
    local_file = work / 'some page & café.html'
    local_file.write_text('<!doctype html><title>Local handoff</title><p>Local bytes</p>')

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            body = ('<!doctype html><title>' + self.path + '</title><p>Launch fixture</p>').encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = f'http://127.0.0.1:{server.server_port}'
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    base = f'http://127.0.0.1:{port}'

    def request(method, path, value=None):
        data = None if value is None else json.dumps(value).encode()
        req = urllib.request.Request(base + path, data=data, method=method, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=10) as response:
            return json.load(response)['value']

    def processes():
        rows = subprocess.check_output(['/bin/ps', '-axo', 'pid=,command='], text=True)
        return {int(pid) for row in rows.splitlines() if len(parts := row.strip().split(maxsplit=1)) == 2
                for pid, command in [parts] if command.startswith(str(executable) + ' ')}

    def wait_for(check):
        until = time.monotonic() + 30
        while True:
            try:
                result = check()
                if result:
                    return result
            except (OSError, ValueError):
                pass
            if time.monotonic() >= until:
                raise AssertionError('Launch Services check timed out')
            time.sleep(0.2)

    register = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'
    try:
        # Cold-start delivery is an Apple event, while --args supplies only test configuration.
        extra_env = [value for name in ['LLVM_PROFILE_FILE', 'GST_PLUGIN_SYSTEM_PATH_1_0']
                     if name in os.environ for value in ['--env', name + '=' + os.environ[name]]]
        subprocess.run(['/usr/bin/open', '-n', '-a', str(app), '--stdout', str(log), '--stderr', str(log), *extra_env, origin + '/cold',
                        '--args', '--profile-dir', str(profile), '--webdriver-port', str(port), 'about:blank'], check=True)
        wait_for(lambda: request('GET', '/status')['ready'])
        session = request('POST', '/session', {'capabilities': {'alwaysMatch': {'browserName': 'wega'}}})
        route = '/session/' + session['sessionId']

        def has_urls(expected):
            found = set()
            for handle in request('GET', route + '/window/handles'):
                request('POST', route + '/window', {'handle': handle})
                address = request('GET', route + '/url')
                parsed = urllib.parse.urlsplit(address)
                if parsed.scheme == 'file':
                    # Launch Services returns decomposed Unicode filenames on
                    # macOS even when the original path used composed accents.
                    path = unicodedata.normalize('NFC', urllib.parse.unquote(parsed.path))
                    address = Path(path).resolve().as_uri()
                if address in expected:
                    title = 'Local handoff' if address.startswith('file:') else urllib.parse.urlsplit(address).path
                    if request('GET', route + '/title') == title:
                        found.add(address)
            return expected <= found

        wait_for(lambda: has_urls({origin + '/cold'}))
        print('Cold-start URL delivery passed', flush=True)
        original = processes()
        assert len(original) == 1, original
        handles = request('GET', route + '/window/handles')
        wanted = {local_file.as_uri(), origin + '/second', origin + '/third'}
        subprocess.run(['/usr/bin/open', '-a', str(app), *sorted(wanted)], check=True)
        wait_for(lambda: has_urls(wanted))
        assert processes() == original, 'URL delivery launched another instance'
        assert len(request('GET', route + '/window/handles')) == len(handles) + 3
        (work / 'result.json').write_text(json.dumps({'passed': True, 'pid': list(original), 'cold': origin + '/cold', 'warm': sorted(wanted)}, indent=2))
        print('PASS: cold URL, existing-instance batch URL and Unicode/space HTML file delivery; ' + str(work))
    except BaseException:
        if 'route' in locals():
            try:
                current = []
                for handle in request('GET', route + '/window/handles'):
                    request('POST', route + '/window', {'handle': handle})
                    current.append((request('GET', route + '/url'), request('GET', route + '/title')))
                print('Test window state: ' + repr(current), file=sys.stderr)
            except OSError:
                pass
        print(log.read_text(), file=sys.stderr)
        raise
    finally:
        for pid in processes():
            os.kill(pid, signal.SIGTERM)
        until = time.monotonic() + 5
        while processes() and time.monotonic() < until:
            time.sleep(0.1)
        for pid in processes():
            os.kill(pid, signal.SIGKILL)
        subprocess.run([register, '-u', str(app)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        shutil.rmtree(app)
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
