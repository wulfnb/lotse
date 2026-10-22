#!/usr/bin/env python3
"""Local authentication gateway. Only nginx's HTTPS listener is public."""
import base64
import hashlib
import hmac
from http.cookies import CookieError, SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
import sys
import threading
import tempfile
import time
from urllib.parse import unquote
from cryptography.fernet import Fernet

DIRECTORY = Path(os.environ.get('BROWSER_AUTH_DIR', '/config/auth'))
LOCK = threading.RLock()
SESSIONS = {}
ATTEMPTS = {}
WEBSOCKET = None
COOKIE = '__Host-servo_session'


def atomic(path, data):
    # A predictable O_TRUNC temporary path follows attacker-created symlinks.
    # mkstemp creates a private, exclusive inode in the destination directory.
    fd, name = tempfile.mkstemp(prefix='.' + path.name + '-', dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def cipher():
    return Fernet((DIRECTORY / 'key').read_bytes())


def decode(value):
    return cipher().decrypt(value[7:].encode()).decode() if value.startswith('enc:v1:') else value


def bootstrap():
    password_file = os.environ.get('BROWSER_PASSWORD_FILE')
    password = Path(password_file).read_text().rstrip('\r\n') if password_file else os.environ.get('BROWSER_PASSWORD', '')
    return decode(os.environ.get('BROWSER_USER', 'servo')), decode(password)


def validate(username, password):
    if not isinstance(username, str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.@-]{0,63}', username):
        raise ValueError('Username must start with a letter or digit and contain 1–64 letters, digits, dots, underscores, @ or hyphens.')
    try:
        password_size = len(password.encode()) if isinstance(password, str) else 0
    except UnicodeError:
        raise ValueError('Password must contain valid UTF-8.') from None
    if not 12 <= password_size <= 128 or any(c in password for c in '\r\n\0'):
        raise ValueError('Password must contain 12–128 UTF-8 bytes without line breaks.')
    if password == 'replace-with-a-unique-password':
        raise ValueError('Replace the example password with your own.')


def digest(password, salt):
    return hashlib.scrypt(password.encode(), salt=salt, n=131072, r=8, p=1, maxmem=256 * 1024 * 1024).hex()


def save(username, password):
    validate(username, password)
    salt = secrets.token_bytes(16)
    record = {'username': username, 'salt': salt.hex(), 'password_hash': digest(password, salt)}
    atomic(DIRECTORY / 'account.enc', cipher().encrypt(json.dumps(record).encode()))


def account():
    return json.loads(cipher().decrypt((DIRECTORY / 'account.enc').read_bytes()))


def matches(record, username, password):
    if not isinstance(username, str) or not isinstance(password, str):
        return False
    try:
        if len(password.encode()) > 1024:
            return False
        username.encode()
    except UnicodeError:
        return False
    valid_password = hmac.compare_digest(record['password_hash'], digest(password, bytes.fromhex(record['salt'])))
    return hmac.compare_digest(record['username'].encode(), username.encode()) and valid_password


def initialize():
    DIRECTORY.mkdir(mode=0o700, parents=True, exist_ok=True)
    DIRECTORY.chmod(0o700)
    if not (DIRECTORY / 'key').exists():
        if (DIRECTORY / 'account.enc').exists():
            raise ValueError('The credential encryption key is missing; restore it from backup.')
        atomic(DIRECTORY / 'key', Fernet.generate_key())
    if not (DIRECTORY / 'account.enc').exists():
        save(*bootstrap())
    account()  # Fail closed if the record/key is corrupt.


def session(handler):
    cookies = SimpleCookie()
    try:
        cookies.load(handler.headers.get('Cookie', ''))
        token = cookies[COOKIE].value
    except (KeyError, ValueError, CookieError):
        return None, None
    value = SESSIONS.get(token)
    if value and value['expires'] > time.monotonic():
        return token, value
    SESSIONS.pop(token, None)
    if value and WEBSOCKET is not None:
        restart_websocket()
    return None, None


def new_session():
    now = time.monotonic()
    revoked = False
    for token in list(SESSIONS):
        if SESSIONS[token]['expires'] <= now:
            del SESSIONS[token]
            revoked = True
    while len(SESSIONS) >= 64:
        del SESSIONS[next(iter(SESSIONS))]
        revoked = True
    if revoked and WEBSOCKET is not None:
        restart_websocket()
    token = secrets.token_urlsafe(32)
    SESSIONS[token] = {'csrf': secrets.token_urlsafe(32), 'expires': now + 12 * 3600}
    return token


def stop_websocket():
    if WEBSOCKET is not None:
        try:
            os.killpg(WEBSOCKET.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            WEBSOCKET.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(WEBSOCKET.pid, signal.SIGKILL)
            WEBSOCKET.wait()
        try:
            os.killpg(WEBSOCKET.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def restart_websocket():
    global WEBSOCKET
    stop_websocket()
    WEBSOCKET = subprocess.Popen(['/usr/bin/websockify', '127.0.0.1:' + os.environ.get('BROWSER_WEBSOCKET_PORT', '6080'), '127.0.0.1:5900'],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)


def watch_websocket():
    while True:
        time.sleep(1)
        with LOCK:
            expired = [token for token, value in SESSIONS.items() if value['expires'] <= time.monotonic()]
            for token in expired:
                del SESSIONS[token]
            if expired:
                restart_websocket()
            if WEBSOCKET.poll() is not None:
                restart_websocket()


UPLOAD_LIMIT = 32 * 1024 * 1024
UPLOAD_QUOTA = 256 * 1024 * 1024


def save_upload(encoded_name, body):
    name = unquote(encoded_name, errors='strict')
    if not name or len(name.encode()) > 200 or name in ('.', '..') or any(c in name for c in '/\\:') or any(ord(c) < 32 for c in name):
        raise ValueError('Choose a file with a valid filename (no paths).')
    if len(body) > UPLOAD_LIMIT:
        raise ValueError('Files must be 32 MiB or smaller.')
    folder = DIRECTORY / 'uploads'
    folder.mkdir(mode=0o700, exist_ok=True)
    files = [p for p in folder.glob('*/*') if p.is_file()]
    if len(files) >= 100 or sum(p.stat().st_size for p in files) + len(body) > UPLOAD_QUOTA:
        raise ValueError('Upload storage is full. Remove unused files from the profile upload folder.')
    private = Path(tempfile.mkdtemp(prefix='file-', dir=folder))
    destination = private / name
    try:
        atomic(destination, body)
    except BaseException:
        private.rmdir()
        raise
    return destination


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass  # Never log headers, bodies or credentials.

    def reply(self, status, value=None, token=None):
        body = b'' if status == 204 else json.dumps(value or {}).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Cache-Control', 'no-store')
        self.send_header('Content-Length', str(len(body)))
        if token is not None:
            self.send_header('Set-Cookie', f'{COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={43200 if token else 0}')
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        with LOCK:
            _, active = session(self)
            if self.path == '/verify':
                return self.reply(204 if active else 401)
            if self.path == '/auth/account':
                if not active:
                    return self.reply(401, {'error': 'Please sign in.'})
                return self.reply(200, {'username': account()['username'], 'csrf': active['csrf']})
            self.reply(404)

    def do_POST(self):
        if self.headers.get('Origin') != 'https://' + self.headers.get('X-Forwarded-Host', ''):
            return self.reply(403, {'error': 'Request origin rejected.'})
        if self.path == '/auth/upload':
            with LOCK:
                _, active = session(self)
                if not active:
                    return self.reply(401, {'error': 'Please sign in.'})
                if not hmac.compare_digest(self.headers.get('X-CSRF-Token', '').encode(), active['csrf'].encode()):
                    return self.reply(403, {'error': 'Reopen Upload and try again.'})
                if self.headers.get('Content-Type') != 'application/octet-stream':
                    return self.reply(415, {'error': 'A binary file is required.'})
                try:
                    length = int(self.headers.get('Content-Length', '-1'))
                    if not 0 <= length <= UPLOAD_LIMIT:
                        return self.reply(413, {'error': 'Files must be 32 MiB or smaller.'})
                    body = self.rfile.read(length)
                    if len(body) != length:
                        return self.reply(400, {'error': 'Upload was incomplete. Please try again.'})
                    path = save_upload(self.headers.get('X-Upload-Name', ''), body)
                    return self.reply(201, {'path': str(path), 'name': path.name})
                except (ValueError, UnicodeError) as error:
                    return self.reply(400, {'error': str(error)})
                except OSError:
                    return self.reply(500, {'error': 'Could not save the file. Check available profile storage.'})
        if self.headers.get('Content-Type', '').split(';')[0] != 'application/json':
            return self.reply(415, {'error': 'JSON required.'})
        try:
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 4096:
                return self.reply(413, {'error': 'Invalid request size.'})
            data = json.loads(self.rfile.read(length))
            if not isinstance(data, dict):
                raise ValueError()
        except (ValueError, UnicodeError):
            return self.reply(400, {'error': 'Invalid request.'})
        with LOCK:
            address = self.headers.get('X-Real-IP') or (self.client_address[0] if hasattr(self, 'client_address') else '127.0.0.1')
            now = time.monotonic()
            for key in list(ATTEMPTS):
                if now - ATTEMPTS[key][0] > 60:
                    del ATTEMPTS[key]
            if len(ATTEMPTS) > 4096:
                return self.reply(429, {'error': 'Please try again later.'})
            if ATTEMPTS.get(address, (now, 0))[1] >= 10:
                return self.reply(429, {'error': 'Too many attempts. Wait one minute.'})
            if self.path == '/auth/login':
                if not matches(account(), data.get('username'), data.get('password')):
                    first, count = ATTEMPTS.get(address, (now, 0))
                    ATTEMPTS[address] = (first, count + 1)
                    return self.reply(401, {'error': 'Incorrect username or password.'})
                ATTEMPTS.pop(address, None)
                return self.reply(200, {'ok': True}, new_session())
            _, active = session(self)
            if not active:
                return self.reply(401, {'error': 'Please sign in.'})
            if not hmac.compare_digest(self.headers.get('X-CSRF-Token', '').encode(), active['csrf'].encode()):
                return self.reply(403, {'error': 'Please reopen account settings and try again.'})
            if self.path == '/auth/logout':
                SESSIONS.clear()
                restart_websocket()
                return self.reply(200, {'ok': True}, '')
            if self.path != '/auth/account':
                return self.reply(404)
            record = account()
            if not matches(record, record['username'], data.get('current_password')):
                first, count = ATTEMPTS.get(address, (now, 0))
                ATTEMPTS[address] = (first, count + 1)
                return self.reply(403, {'error': 'Current password is incorrect.'})
            try:
                validate(data.get('username'), data.get('password'))
                if data['password'] != data.get('confirm_password'):
                    raise ValueError('New passwords do not match.')
                save(data['username'], data['password'])
            except ValueError as error:
                return self.reply(400, {'error': str(error)})
            SESSIONS.clear()
            ATTEMPTS.pop(address, None)
            restart_websocket()  # Close existing control connections, not only HTTP sessions.
            self.reply(200, {'ok': True}, new_session())


if __name__ == '__main__':
    try:
        if len(sys.argv) > 1 and sys.argv[1] == 'initialize':
            initialize()
        elif len(sys.argv) > 1 and sys.argv[1] == 'encrypt-bootstrap':
            values = json.load(sys.stdin)
            print(json.dumps({key: 'enc:v1:' + cipher().encrypt(value.encode()).decode() for key, value in values.items()}))
        elif len(sys.argv) > 1 and sys.argv[1] == 'show-bootstrap':
            username, password = bootstrap()
            print('Initial username:', username)
            print('Initial password:', password)
            print('These are initial credentials only; a password changed in Account cannot be recovered.')
        elif len(sys.argv) > 1 and sys.argv[1] == 'bootstrap-json':
            username, password = bootstrap()
            print(json.dumps({'username': username, 'password': password}))
        elif len(sys.argv) > 1 and sys.argv[1] == 'encrypt-current-bootstrap':
            username, password = bootstrap()
            print(json.dumps({key: 'enc:v1:' + cipher().encrypt(value.encode()).decode()
                              for key, value in {'SERVO_WEB_USER': username, 'SERVO_WEB_PASSWORD': password}.items()}))
        else:
            initialize()
            restart_websocket()
            threading.Thread(target=watch_websocket, daemon=True).start()
            signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
            signal.signal(signal.SIGINT, lambda *_: sys.exit(0))
            try:
                ThreadingHTTPServer(('127.0.0.1', int(os.environ.get('BROWSER_AUTH_PORT', '6081'))), Handler).serve_forever()
            finally:
                stop_websocket()
    except (ValueError, OSError) as error:
        sys.exit('Web authentication could not start: ' + str(error))
