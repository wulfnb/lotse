#!/usr/bin/env python3
"""Isolated credential/session tests; never changes the real browser account."""
import http.client
from http.server import ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import secrets
import socket
import tempfile
import threading
import time

auth_path = Path(os.environ.get('LOTSE_AUTH_MODULE', '/opt/servo/auth-service.py'))
if not auth_path.exists():
    auth_path = Path(__file__).resolve().parents[1] / 'docker' / 'auth-service.py'
spec = importlib.util.spec_from_file_location('auth', auth_path)
auth = importlib.util.module_from_spec(spec)
spec.loader.exec_module(auth)

with tempfile.TemporaryDirectory(prefix='servo-auth-test-') as temporary:
    auth.DIRECTORY = Path(temporary)
    username = 'test-' + secrets.token_hex(8)
    password = secrets.token_urlsafe(32)
    os.environ.update(BROWSER_USER=username, BROWSER_PASSWORD=password, BROWSER_WEBSOCKET_PORT='6084')
    os.environ.pop('BROWSER_PASSWORD_FILE', None)
    auth.initialize()
    assert username.encode() not in (auth.DIRECTORY / 'account.enc').read_bytes()
    assert password.encode() not in (auth.DIRECTORY / 'account.enc').read_bytes()
    for name in ('key', 'account.enc'):
        assert (auth.DIRECTORY / name).stat().st_mode & 0o777 == 0o600
    assert auth.DIRECTORY.stat().st_mode & 0o777 == 0o700
    assert 'password' not in auth.account() and auth.account()['password_hash'] != password
    auth.restart_websocket()
    server = ThreadingHTTPServer(('127.0.0.1', 0), auth.Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    origin = 'https://auth-check.local:7443'

    def request(path, data=None, cookie=None, csrf=None, request_origin=origin):
        headers = {'Origin': request_origin, 'X-Forwarded-Host': 'auth-check.local:7443', 'X-Real-IP': 'test'}
        if cookie:
            headers['Cookie'] = cookie
        if csrf is not None:
            headers['X-CSRF-Token'] = csrf
        if data is not None:
            headers['Content-Type'] = 'application/json'
        connection = http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=10)
        connection.request('POST' if data is not None else 'GET', path, json.dumps(data) if data is not None else None, headers)
        response = connection.getresponse()
        body = response.read()
        result = (response.status, json.loads(body) if body else {}, (response.getheader('Set-Cookie') or '').split(';', 1)[0], response.getheader('Set-Cookie') or '')
        connection.close()
        return result

    def login(user=username, secret=password):
        status, _, cookie, flags = request('/auth/login', {'username': user, 'password': secret})
        assert status == 200
        assert all(flag in flags for flag in ('Secure', 'HttpOnly', 'SameSite=Strict', 'Path=/'))
        return cookie

    try:
        assert request('/verify')[0] == 401
        assert request('/auth/login', {'username': username, 'password': password}, request_origin='https://evil.invalid')[0] == 403
        assert request('/auth/login', {'username': username, 'password': 'wrong'})[0] == 401
        first = login()
        second = login()
        assert request('/verify', cookie=first)[0] == 204
        _, details, _, _ = request('/auth/account', cookie=first)
        csrf = details['csrf']
        assert details['username'] == username
        new_username, new_password = 'new-' + secrets.token_hex(8), secrets.token_urlsafe(32)
        change = {'username': new_username, 'password': new_password, 'confirm_password': new_password, 'current_password': password}
        assert request('/auth/account', change, first)[0] == 403
        assert request('/auth/account', change, first, csrf, request_origin='https://evil.invalid')[0] == 403
        assert request('/auth/account', dict(change, current_password='wrong'), first, csrf)[0] == 403
        assert request('/auth/account', dict(change, confirm_password='mismatch'), first, csrf)[0] == 400
        assert request('/auth/account', dict(change, password='short', confirm_password='short'), first, csrf)[0] == 400
        assert auth.matches(auth.account(), username, password)
        # A real WebSocket/RFB connection must be closed when credentials change.
        control = None
        for _ in range(50):
            try:
                control = socket.create_connection(('127.0.0.1', 6084), timeout=1)
                break
            except OSError:
                time.sleep(.1)
        assert control is not None
        control.sendall(b'GET / HTTP/1.1\r\nHost: localhost:6084\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: binary\r\n\r\n')
        assert b'101' in control.recv(4096)
        control.settimeout(.2)
        try:
            while control.recv(4096):
                pass
        except socket.timeout:
            pass
        status, _, rotated, _ = request('/auth/account', change, first, csrf)
        assert status == 200
        control.settimeout(5)
        while control.recv(4096):
            pass
        control.close()
        assert request('/verify', cookie=first)[0] == 401
        assert request('/verify', cookie=second)[0] == 401
        assert request('/verify', cookie=rotated)[0] == 204
        assert request('/auth/login', {'username': username, 'password': password})[0] == 401
        latest = login(new_username, new_password)
        auth.initialize()  # Stored account wins over old bootstrap credentials after restart.
        assert auth.matches(auth.account(), new_username, new_password)
        assert new_username.encode() not in (auth.DIRECTORY / 'account.enc').read_bytes()
        assert new_password.encode() not in (auth.DIRECTORY / 'account.enc').read_bytes()
        _, details, _, _ = request('/auth/account', cookie=latest)
        assert request('/auth/logout', {}, latest, details['csrf'])[0] == 200
        assert request('/verify', cookie=latest)[0] == 401
        assert request('/verify', cookie=rotated)[0] == 401
        auth.ATTEMPTS.clear()
        for _ in range(10):
            assert request('/auth/login', {'username': 'wrong', 'password': 'wrong'})[0] == 401
        assert request('/auth/login', {'username': 'wrong', 'password': 'wrong'})[0] == 429
        encrypted_user = 'enc:v1:' + auth.cipher().encrypt(new_username.encode()).decode()
        encrypted_password = 'enc:v1:' + auth.cipher().encrypt(new_password.encode()).decode()
        assert auth.decode(encrypted_user) == new_username and auth.decode(encrypted_password) == new_password
        print('PASS: encrypted storage/bootstrap, password hashing, login/session flags, current-password verification, CSRF/origin checks, validation, credential rotation, old-session rejection, active WebSocket revocation, persistence, logout and rate limiting')
    finally:
        server.shutdown()
        auth.stop_websocket()
