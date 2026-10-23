#!/usr/bin/env python3
"""Local authentication gateway. Only nginx is public: its HTTPS listener, plus
an optional plain-HTTP one (WEGA_HTTP_PORT) for a TLS-terminating proxy.

The gateway and its WebSocket bridge listen on Unix sockets, never on TCP
loopback: pages loaded by the browser in this container can open loopback TCP
connections, but they cannot open Unix sockets.
"""
import base64
import hashlib
import hmac
from http.cookies import CookieError, SimpleCookie
from http.server import BaseHTTPRequestHandler
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import socketserver
import subprocess
import sys
import threading
import tempfile
import time
from urllib.parse import unquote
from cryptography.fernet import Fernet

DIRECTORY = Path(os.environ.get('BROWSER_AUTH_DIR', '/config/auth'))
UPLOADS = Path(os.environ.get('BROWSER_UPLOAD_DIR', '/config/uploads'))
CHOOSER = Path(os.environ.get('WEGA_FILE_HANDOFF_DIR', '/run/wega/chooser'))
AUTH_SOCKET = os.environ.get('BROWSER_AUTH_SOCKET', '/run/wega/auth/auth.sock')
WEBSOCKET_SOCKET = os.environ.get('BROWSER_WEBSOCKET_SOCKET', '/run/wega/auth/websocket.sock')
VNC_SOCKET = os.environ.get('BROWSER_VNC_SOCKET', '/run/wega/vnc/rfb.sock')
LOCK = threading.RLock()
# scrypt(N=2^17, r=8) needs 128 MiB per hash. Hashing runs outside LOCK, so
# bound concurrency to keep parallel login attempts within the memory limit.
HASHING = threading.BoundedSemaphore(2)
UPLOAD_LOCK = threading.Lock()
SESSIONS = {}
ATTEMPTS = {}
ATTEMPT_LIMIT = 10
ATTEMPT_TABLE = 4096
WEBSOCKET = None
COOKIE = '__Host-wega_session'


def atomic(path, data, mode=0o600):
    # A predictable O_TRUNC temporary path follows attacker-created symlinks.
    # mkstemp creates a private, exclusive inode in the destination directory.
    fd, name = tempfile.mkstemp(prefix='.' + path.name + '-', dir=path.parent)
    temporary = Path(name)
    try:
        if mode != 0o600:
            os.fchmod(fd, mode)
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
    with HASHING:
        return hashlib.scrypt(password.encode(), salt=salt, n=131072, r=8, p=1, maxmem=256 * 1024 * 1024).hex()


def prepare(username, password):
    """Validate and hash a new account record; slow, so call without LOCK."""
    validate(username, password)
    salt = secrets.token_bytes(16)
    return {'username': username, 'salt': salt.hex(), 'password_hash': digest(password, salt)}


def store(record):
    atomic(DIRECTORY / 'account.enc', cipher().encrypt(json.dumps(record).encode()))


def save(username, password):
    store(prepare(username, password))


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
    # Unix sockets only: nginx (after auth_request) is the sole client, and
    # x11vnc listens on a Unix socket without any TCP port.
    WEBSOCKET = subprocess.Popen(['/usr/bin/websockify', '--unix-listen=' + WEBSOCKET_SOCKET, '--unix-listen-mode=660',
                                  '--unix-target=' + VNC_SOCKET],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)


def watch_websocket():
    pruned = time.monotonic()
    while True:
        time.sleep(1)
        if time.monotonic() - pruned >= 3600:
            pruned = time.monotonic()
            prune_uploads()
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


def retention_days():
    try:
        return max(0, int(os.environ.get('WEGA_UPLOAD_RETENTION_DAYS', '7')))
    except ValueError:
        return 7


def prune_uploads(now=None):
    """Delete upload folders older than WEGA_UPLOAD_RETENTION_DAYS (0 keeps all)."""
    days = retention_days()
    if not days or not UPLOADS.is_dir():
        return 0
    cutoff = (time.time() if now is None else now) - days * 86400
    removed = 0
    for entry in UPLOADS.iterdir():
        try:
            if entry.is_symlink() or not entry.is_dir() or entry.lstat().st_mtime >= cutoff:
                continue
            shutil.rmtree(entry)
            removed += 1
        except OSError:
            pass  # Best effort; a later pass retries.
    return removed


def save_upload(encoded_name, body):
    name = unquote(encoded_name, errors='strict')
    if not name or len(name.encode()) > 200 or name in ('.', '..') or any(c in name for c in '/\\:') or any(ord(c) < 32 for c in name):
        raise ValueError('Choose a file with a valid filename (no paths).')
    if len(body) > UPLOAD_LIMIT:
        raise ValueError('Files must be 32 MiB or smaller.')
    # Outside the secret auth directory: the browser UID (group) must read it
    # to pass the file to a website's file chooser. Entrypoint makes the folder
    # setgid with the browser group, so new entries inherit that group.
    folder = UPLOADS
    folder.mkdir(mode=0o750, exist_ok=True)
    files = [p for p in folder.glob('*/*') if p.is_file()]
    if len(files) >= 100 or sum(p.stat().st_size for p in files) + len(body) > UPLOAD_QUOTA:
        raise ValueError('Upload storage is full. Remove unused files from the profile upload folder.')
    while True:
        # Not mkdtemp + chmod: chmod by a non-member of the folder's group
        # clears the inherited setgid bit, and new files would lose the
        # browser group. mkdir keeps it; the 0750 mode is limited by umask.
        private = folder / ('file-' + secrets.token_hex(8))
        try:
            private.mkdir(mode=0o750)
            break
        except FileExistsError:
            continue
    destination = private / name
    try:
        atomic(destination, body, 0o640)
    except BaseException:
        private.rmdir()
        raise
    return destination


def chooser_request():
    try:
        raw = (CHOOSER / 'current.json').read_bytes()
        if len(raw) > 8192:
            return None
        request = json.loads(raw)
        if not isinstance(request, dict) or not re.fullmatch(r'[0-9a-f]{32}', request.get('id', '')):
            return None
        if not isinstance(request.get('expires'), int) or not time.time() <= request['expires'] <= time.time() + 600:
            return None
        if not isinstance(request.get('multiple'), bool) or not isinstance(request.get('origin'), str):
            return None
        if (CHOOSER / ('accepted-' + request['id'])).exists():
            return None
        extensions = request.get('extensions')
        if not isinstance(extensions, list) or not all(isinstance(item, str) and len(item) <= 256 for item in extensions):
            return None
        return request
    except (OSError, ValueError, TypeError):
        return None


def choose_uploaded_files(data):
    request = chooser_request()
    if not request or data.get('id') != request['id']:
        raise ValueError('This file chooser has closed or expired. Reopen it in the browser.')
    paths = data.get('paths')
    if not isinstance(paths, list) or len(paths) > 32 or (not request['multiple'] and len(paths) > 1):
        raise ValueError('Invalid number of selected files.')
    root = UPLOADS.resolve()
    selected = []
    for value in paths:
        if not isinstance(value, str):
            raise ValueError('Invalid uploaded file path.')
        path = Path(value).resolve()
        try:
            relative = path.relative_to(root)
        except ValueError:
            raise ValueError('Only uploaded files can be selected.') from None
        if len(relative.parts) != 2 or not relative.parts[0].startswith('file-') or not path.is_file():
            raise ValueError('The uploaded file is unavailable.')
        selected.append(str(path))
    # First terminal action wins: a late upload cannot overwrite cancellation.
    marker = CHOOSER / ('accepted-' + request['id'])
    try:
        fd = os.open(marker, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o640)
    except FileExistsError:
        raise ValueError('This file chooser was already answered.') from None
    os.close(fd)
    try:
        atomic(CHOOSER / ('reply-' + request['id'] + '.json'), json.dumps({'id': request['id'], 'paths': selected}).encode(), 0o640)
    except BaseException:
        marker.unlink(missing_ok=True)
        raise


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
            if self.path == '/auth/chooser':
                if not active:
                    return self.reply(401, {'error': 'Please sign in.'})
                return self.reply(200, {'request': chooser_request()})
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
            # The body is read without LOCK: a slow upload must not stall
            # /verify for every other viewer request.
            if self.headers.get('Content-Type') != 'application/octet-stream':
                return self.reply(415, {'error': 'A binary file is required.'})
            try:
                length = int(self.headers.get('Content-Length', '-1'))
                if not 0 <= length <= UPLOAD_LIMIT:
                    return self.reply(413, {'error': 'Files must be 32 MiB or smaller.'})
                body = self.rfile.read(length)
                if len(body) != length:
                    return self.reply(400, {'error': 'Upload was incomplete. Please try again.'})
                with UPLOAD_LOCK:  # Serialise quota checks and publication.
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
            if not 0 < length <= (16384 if self.path == '/auth/chooser' else 4096):
                return self.reply(413, {'error': 'Invalid request size.'})
            data = json.loads(self.rfile.read(length))
            if not isinstance(data, dict):
                raise ValueError()
        except (ValueError, UnicodeError):
            return self.reply(400, {'error': 'Invalid request.'})
        # nginx sets X-Real-IP from $remote_addr (after optional trusted-proxy
        # real_ip processing). Only nginx can reach the gateway's Unix socket.
        address = self.headers.get('X-Real-IP') or 'local'
        with LOCK:
            if not charge_attempt(address):
                return self.reply(429, {'error': 'Too many attempts. Wait one minute.'})
            if self.path == '/auth/login':
                record = account()
            else:
                token, active = session(self)
                if not active:
                    refund_attempt(address)
                    return self.reply(401, {'error': 'Please sign in.'})
                if not hmac.compare_digest(self.headers.get('X-CSRF-Token', '').encode(), active['csrf'].encode()):
                    refund_attempt(address)
                    return self.reply(403, {'error': 'Please reopen account settings and try again.'})
                if self.path == '/auth/chooser':
                    refund_attempt(address)
                    try:
                        choose_uploaded_files(data)
                    except (ValueError, OSError) as error:
                        return self.reply(400, {'error': str(error)})
                    return self.reply(200, {'ok': True})
                if self.path == '/auth/logout':
                    refund_attempt(address)
                    SESSIONS.clear()
                    restart_websocket()
                    return self.reply(200, {'ok': True}, '')
                if self.path != '/auth/account':
                    refund_attempt(address)
                    return self.reply(404)
                record = account()
        # scrypt runs without LOCK so one slow hash never blocks /verify and
        # every other viewer request. The attempt was charged up front, so
        # parallel guesses cannot exceed the per-address limit.
        if self.path == '/auth/login':
            if not matches(record, data.get('username'), data.get('password')):
                return self.reply(401, {'error': 'Incorrect username or password.'})
            with LOCK:
                ATTEMPTS.pop(address, None)
                return self.reply(200, {'ok': True}, new_session())
        if not matches(record, record['username'], data.get('current_password')):
            return self.reply(403, {'error': 'Current password is incorrect.'})
        refund_attempt(address)
        try:
            validate(data.get('username'), data.get('password'))
            if data['password'] != data.get('confirm_password'):
                raise ValueError('New passwords do not match.')
            replacement = prepare(data['username'], data['password'])
        except ValueError as error:
            return self.reply(400, {'error': str(error)})
        with LOCK:
            if token not in SESSIONS:
                return self.reply(401, {'error': 'Please sign in.'})
            store(replacement)
            SESSIONS.clear()
            ATTEMPTS.pop(address, None)
            restart_websocket()  # Close existing control connections, not only HTTP sessions.
            self.reply(200, {'ok': True}, new_session())


def charge_attempt(address):
    """Reserve one authentication attempt for address; call with LOCK held."""
    now = time.monotonic()
    for key in list(ATTEMPTS):
        if now - ATTEMPTS[key][0] > 60:
            del ATTEMPTS[key]
    first, count = ATTEMPTS.get(address, (now, 0))
    if count >= ATTEMPT_LIMIT:
        return False
    if address not in ATTEMPTS:
        # Evict the oldest windows instead of refusing every client when many
        # distinct addresses are tracked.
        while len(ATTEMPTS) >= ATTEMPT_TABLE:
            del ATTEMPTS[next(iter(ATTEMPTS))]
    ATTEMPTS[address] = (first, count + 1)
    return True


def refund_attempt(address):
    with LOCK:
        if address in ATTEMPTS:
            first, count = ATTEMPTS[address]
            if count <= 1:
                del ATTEMPTS[address]
            else:
                ATTEMPTS[address] = (first, count - 1)


class UnixHTTPServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True
    block_on_close = False
    # nginx sends one /verify per asset, so a page load arrives as a burst of ~40;
    # socketserver's default backlog of 5 refuses the rest with EAGAIN.
    request_queue_size = 128

    def __init__(self, path, handler):
        Path(path).unlink(missing_ok=True)
        super().__init__(path, handler)
        os.chmod(path, 0o660)  # nginx (browser group via wega-ipc) connects.

    def get_request(self):
        request, _ = self.socket.accept()
        return request, ('local', 0)

    def server_close(self):
        super().server_close()
        Path(self.server_address).unlink(missing_ok=True)


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
                              for key, value in {'WEGA_WEB_USER': username, 'WEGA_WEB_PASSWORD': password}.items()}))
        else:
            initialize()
            prune_uploads()
            restart_websocket()
            threading.Thread(target=watch_websocket, daemon=True).start()
            signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
            signal.signal(signal.SIGINT, lambda *_: sys.exit(0))
            server = UnixHTTPServer(AUTH_SOCKET, Handler)
            try:
                server.serve_forever()
            finally:
                server.server_close()
                stop_websocket()
    except (ValueError, OSError) as error:
        sys.exit('Web authentication could not start: ' + str(error))
