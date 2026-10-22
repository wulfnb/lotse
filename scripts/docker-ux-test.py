#!/usr/bin/env python3
"""Exercise the current viewer/gateway through nginx in an isolated container.

Uses an existing image for system dependencies; mounts the current gateway,
viewer and nginx files. This checks those layers, not a new Linux Rust build.
"""
import http.client
import json
from pathlib import Path
import ssl
import sys
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]


def docker(*args):
    return subprocess.check_output(['docker', *args], text=True).strip()


def main():
    name = 'lotse-ux-' + uuid.uuid4().hex[:10]
    container = docker('run', '--rm', '-d', '--name', name, '-p', '127.0.0.1::7443',
                       '-e', 'BROWSER_USER=ux-test', '-e', 'BROWSER_PASSWORD=isolated-ux-test-password',
                       '-v', f'{ROOT / "docker/auth-service.py"}:/opt/servo/auth-service.py:ro',
                       '-v', f'{ROOT / "docker/viewer"}:/opt/servo/viewer:ro',
                       '-v', f'{ROOT / "docker/nginx.conf"}:/etc/nginx/nginx.conf:ro',
                       sys.argv[1] if len(sys.argv) > 1 else 'servo-browser:local')
    try:
        port = int(docker('port', container, '7443/tcp').rsplit(':', 1)[1])
        host = f'127.0.0.1:{port}'

        def request(path, body=None, headers=None):
            connection = http.client.HTTPSConnection('127.0.0.1', port, timeout=10,
                                                    context=ssl._create_unverified_context())
            try:
                connection.request('GET' if body is None else 'POST', path, body=body, headers=headers or {})
                response = connection.getresponse()
                return response.status, response.read(), response.getheader('Set-Cookie')
            finally:
                connection.close()

        for attempt in range(60):
            try:
                if request('/healthz')[0] == 200:
                    break
            except (OSError, http.client.HTTPException):
                pass
            if attempt == 59:
                raise AssertionError('Isolated gateway did not start')
            time.sleep(0.25)
        docker('exec', container, 'nginx', '-t', '-e', 'stderr')
        origin = {'Origin': f'https://{host}'}
        assert request('/auth/upload', b'file', origin)[0] == 401
        login = json.dumps({'username': 'ux-test', 'password': 'isolated-ux-test-password'}).encode()
        status, _, cookie = request('/auth/login', login, {**origin, 'Content-Type': 'application/json'})
        assert status == 200
        cookie = cookie.split(';', 1)[0]
        session = {'Cookie': cookie}
        account = json.loads(request('/auth/account', headers=session)[1])
        headers = {**origin, **session, 'X-CSRF-Token': account['csrf'],
                   'Content-Type': 'application/octet-stream', 'X-Upload-Name': 'sample%20file.bin'}
        assert request('/auth/upload', b'file', {**headers, 'X-CSRF-Token': 'invalid'})[0] == 403
        assert request('/auth/upload', b'file', {**headers, 'Origin': 'https://elsewhere.invalid'})[0] == 403
        assert request('/auth/upload', b'', {**headers, 'Content-Length': str(33 * 1024 * 1024)})[0] == 413
        status, body, _ = request('/auth/upload', b'private binary\x00content', headers)
        assert status == 201, (status, body)
        path = json.loads(body)['path']
        # The native browser UID must be able to select the file uploaded by the viewer.
        assert docker('exec', '--user', 'browser', container, 'python3', '-c',
                      'from pathlib import Path; import sys; assert Path(sys.argv[1]).read_bytes()==b"private binary\\x00content"; print("readable")',
                      path) == 'readable'
        assert request('/auth/uploads/')[0] != 200
        assert request('/index.html')[0] == 401
        page = request('/index.html', headers=session)[1]
        assert b'id="upload"' in page and b'id="viewport-mode"' in page
        script = request('/viewer.js', headers=session)[1]
        assert b'/auth/upload' in script and b'resizeSession' in script
        print('PASS: nginx syntax, authentication, CSRF/origin/size guards, private file handoff and current viewer assets')
    finally:
        subprocess.run(['docker', 'rm', '-f', container], check=True, stdout=subprocess.DEVNULL)


if __name__ == '__main__':
    main()
