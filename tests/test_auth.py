"""Portable tests of the real gateway; no Docker, network listeners or real account."""
from email.message import Message
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('lotse_auth', Path(__file__).resolve().parents[1] / 'docker/auth-service.py')
auth = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(auth)
RESTART_WEBSOCKET = auth.restart_websocket


class AuthHelpers:
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='lotse-auth-unit-')
        self.addCleanup(self.temporary.cleanup)
        self.environment = patch.dict(os.environ, {'BROWSER_USER': 'alice', 'BROWSER_PASSWORD': 'a-unique-test-password'}, clear=True)
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.directory = patch.object(auth, 'DIRECTORY', Path(self.temporary.name))
        self.directory.start()
        self.addCleanup(self.directory.stop)
        self.websocket = patch.object(auth, 'restart_websocket')
        self.restart = self.websocket.start()
        self.addCleanup(self.websocket.stop)
        auth.SESSIONS.clear()
        auth.ATTEMPTS.clear()
        auth.initialize()

    def request(self, path, data=None, token=None, csrf=None, origin='https://test.local', content_type='application/json', raw=None, length=None, upload_name=None):
        handler = object.__new__(auth.Handler)
        handler.path = path
        handler.headers = Message()
        handler.headers['Origin'] = origin
        handler.headers['X-Forwarded-Host'] = 'test.local'
        handler.headers['X-Real-IP'] = 'test-client'
        handler.headers['Content-Type'] = content_type
        if upload_name is not None:
            handler.headers['X-Upload-Name'] = upload_name
        if token:
            handler.headers['Cookie'] = f'{auth.COOKIE}={token}'
        if csrf:
            handler.headers['X-CSRF-Token'] = csrf
        body = raw if raw is not None else json.dumps(data).encode()
        handler.headers['Content-Length'] = str(length if length is not None else len(body))
        handler.rfile = io.BytesIO(body)
        result = []
        handler.reply = lambda status, value=None, token=None: result.append((status, value, token))
        if data is None and raw is None:
            handler.do_GET()
        else:
            handler.do_POST()
        return result[0]

    def login(self):
        status, _, token = self.request('/auth/login', {'username': 'alice', 'password': 'a-unique-test-password'})
        self.assertEqual(status, 200)
        return token

class AuthTests(AuthHelpers, unittest.TestCase):
    def test_encrypted_storage_permissions_and_persistence(self):
        record = auth.account()
        self.assertNotIn('password', record)
        self.assertNotEqual(record['password_hash'], 'a-unique-test-password')
        encrypted = (auth.DIRECTORY / 'account.enc').read_bytes()
        self.assertNotIn(b'alice', encrypted)
        self.assertNotIn(b'a-unique-test-password', encrypted)
        auth.initialize()
        self.assertEqual(auth.account(), record)
        if os.name != 'nt':
            self.assertEqual(auth.DIRECTORY.stat().st_mode & 0o777, 0o700)
            self.assertEqual((auth.DIRECTORY / 'account.enc').stat().st_mode & 0o777, 0o600)
        value = 'enc:v1:' + auth.cipher().encrypt(b'value').decode()
        self.assertEqual(auth.decode(value), 'value')
        self.assertEqual(auth.decode('plain'), 'plain')

    def test_missing_key_and_corruption_fail_closed(self):
        (auth.DIRECTORY / 'key').unlink()
        with self.assertRaises(ValueError):
            auth.initialize()

    def test_invalid_credentials_and_validation(self):
        record = auth.account()
        for username, password in [(None, None), ('alice', '\ud800'), ('alice', 'x' * 1025), ('wrong', 'a-unique-test-password')]:
            self.assertFalse(auth.matches(record, username, password))
        for username, password in [('', 'a-unique-test-password'), ('bad user', 'a-unique-test-password'), ('alice', 'short'), ('alice', 'replace-with-a-unique-password'), ('alice', 'a-long-password\n')]:
            with self.assertRaises(ValueError):
                auth.validate(username, password)

    def test_origin_json_size_and_session_guards(self):
        self.assertEqual(self.request('/verify')[0], 401)
        self.assertEqual(self.request('/auth/account')[0], 401)
        self.assertEqual(self.request('/missing')[0], 404)
        cases = [({'origin': 'https://evil.test'}, 403), ({'content_type': 'text/plain'}, 415), ({'length': 5000}, 413), ({'length': -1}, 413), ({'raw': b'not-json'}, 400), ({'raw': b'[]'}, 400)]
        for options, expected in cases:
            self.assertEqual(self.request('/auth/login', {}, **options)[0], expected)
        self.assertEqual(self.request('/auth/account', {})[0], 401)
        token = self.login()
        self.assertEqual(self.request('/verify', token=token)[0], 204)
        self.assertEqual(self.request('/auth/logout', {}, token=token)[0], 403)

    def test_credential_rotation_revokes_sessions_and_requires_current_password(self):
        first, second = self.login(), self.login()
        csrf = auth.SESSIONS[first]['csrf']
        change = {'username': 'bob', 'password': 'new-unique-test-password', 'confirm_password': 'new-unique-test-password', 'current_password': 'a-unique-test-password'}
        self.assertEqual(self.request('/auth/account', dict(change, current_password='wrong'), first, csrf)[0], 403)
        self.assertEqual(self.request('/auth/account', dict(change, confirm_password='different'), first, csrf)[0], 400)
        status, _, replacement = self.request('/auth/account', change, first, csrf)
        self.assertEqual(status, 200)
        self.assertNotIn(first, auth.SESSIONS)
        self.assertNotIn(second, auth.SESSIONS)
        self.assertIn(replacement, auth.SESSIONS)
        self.restart.assert_called()
        self.assertTrue(auth.matches(auth.account(), 'bob', 'new-unique-test-password'))
        details = self.request('/auth/account', token=replacement)[1]
        self.assertEqual(details['username'], 'bob')
        self.assertEqual(self.request('/missing', {}, replacement, details['csrf'])[0], 404)
        self.assertEqual(self.request('/auth/logout', {}, replacement, details['csrf'])[0], 200)
        self.assertFalse(auth.SESSIONS)

    def test_expiration_bounded_sessions_and_rate_limit(self):
        token = self.login()
        auth.SESSIONS[token]['expires'] = time.monotonic() - 1
        self.assertEqual(self.request('/verify', token=token)[0], 401)
        for _ in range(66):
            auth.new_session()
        self.assertEqual(len(auth.SESSIONS), 64)
        for _ in range(10):
            self.assertEqual(self.request('/auth/login', {'username': 'alice', 'password': 'wrong'})[0], 401)
        self.assertEqual(self.request('/auth/login', {'username': 'alice', 'password': 'wrong'})[0], 429)
        auth.ATTEMPTS['test-client'] = (time.monotonic() - 61, 10)
        self.login()

    def test_reply_sets_secure_cookie_flags(self):
        handler = object.__new__(auth.Handler)
        headers = {}
        handler.send_response = lambda status: headers.update(status=status)
        handler.send_header = lambda key, value: headers.update({key: value})
        handler.end_headers = lambda: None
        handler.wfile = io.BytesIO()
        handler.reply(200, {'ok': True}, 'token')
        for flag in ['Secure', 'HttpOnly', 'SameSite=Strict', 'Path=/', 'Max-Age=43200']:
            self.assertIn(flag, headers['Set-Cookie'])
        self.assertEqual(headers['Cache-Control'], 'no-store')
        self.assertEqual(json.loads(handler.wfile.getvalue()), {'ok': True})


class AuthFailureAndProcessTests(AuthHelpers, unittest.TestCase):
    # Exercise the real OS boundary functions with test
    # doubles. No signal is delivered and no subprocess/listener is started.
    def test_password_file_and_malformed_cookie(self):
        secret = auth.DIRECTORY / 'password-input'
        secret.write_text('file-secret-password\r\n')
        with patch.dict(os.environ, {'BROWSER_PASSWORD_FILE': str(secret)}):
            self.assertEqual(auth.bootstrap(), ('alice', 'file-secret-password'))
        handler = object.__new__(auth.Handler)
        handler.headers = {'Cookie': 'not-a-cookie'}
        self.assertEqual(auth.session(handler), (None, None))
        with patch.object(auth, 'WEBSOCKET', object()):
            token = self.login()
            auth.SESSIONS[token]['expires'] = time.monotonic() - 1
            self.assertEqual(self.request('/verify', token=token)[0], 401)
            self.restart.assert_called()
            token = auth.new_session()
            auth.SESSIONS[token]['expires'] = 0
            self.assertNotEqual(auth.new_session(), token)

    def test_watchdog_revokes_expired_sessions_and_restarts_dead_bridge(self):
        from unittest.mock import Mock
        for expired, exited in [(True, False), (False, True), (False, False)]:
            auth.SESSIONS.clear()
            token = auth.new_session()
            if expired:
                auth.SESSIONS[token]['expires'] = 0
            process = Mock()
            process.poll.return_value = 1 if exited else None
            self.restart.reset_mock()
            with patch.object(auth, 'WEBSOCKET', process), patch.object(auth.time, 'sleep', side_effect=[None, StopIteration]):
                with self.assertRaises(StopIteration):
                    auth.watch_websocket()
            self.assertEqual(self.restart.called, expired or exited)
            self.assertEqual(token in auth.SESSIONS, not expired)

    def test_bridge_shutdown_handles_timeout_and_process_exit_races(self):
        from unittest.mock import Mock
        for first_race, final_race, timed_out in [(False, False, False), (True, True, False), (False, False, True)]:
            process = Mock(pid=123456)
            if timed_out:
                process.wait.side_effect = [auth.subprocess.TimeoutExpired('websockify', 3), None]
            effects = [ProcessLookupError() if first_race else None]
            if timed_out:
                effects.append(None)
            effects.append(ProcessLookupError() if final_race else None)
            with patch.object(auth, 'WEBSOCKET', process), patch.object(auth.os, 'killpg', side_effect=effects) as kill:
                auth.stop_websocket()
                self.assertEqual(kill.call_count, 3 if timed_out else 2)
                process.wait.assert_called()
        with patch.object(auth, 'WEBSOCKET', None), patch.object(auth.os, 'killpg') as kill:
            auth.stop_websocket()
            kill.assert_not_called()

    def test_restart_launches_only_loopback_bridge_in_its_own_process_group(self):
        from unittest.mock import Mock
        process = Mock()
        with patch.object(auth.subprocess, 'Popen', return_value=process) as spawn, patch.object(auth, 'WEBSOCKET', None), patch.object(auth, 'stop_websocket') as stop:
            RESTART_WEBSOCKET()
            self.assertEqual(auth.WEBSOCKET, process)
            stop.assert_called_once()
            self.assertTrue(spawn.call_args.kwargs['start_new_session'])
            self.assertIn('127.0.0.1:6080', spawn.call_args.args[0])

    def test_attempt_table_saturation_cookie_reply_and_silent_logging(self):
        auth.ATTEMPTS.update({str(index): (time.monotonic(), 1) for index in range(4097)})
        self.assertEqual(self.request('/auth/login', {})[0], 429)
        handler = object.__new__(auth.Handler)
        handler.send_response = lambda *_: None
        handler.send_header = lambda *_: None
        handler.end_headers = lambda: None
        handler.wfile = io.BytesIO()
        handler.reply(204)
        self.assertEqual(handler.wfile.getvalue(), b'')
        handler.log_message('sensitive message')
        auth.ATTEMPTS.clear()
        with patch.object(auth, 'WEBSOCKET', object()):
            auth.new_session()
            for _ in range(64):
                auth.new_session()
            self.restart.assert_called()

    def test_entrypoint_bootstrap_commands_and_failure_are_isolated(self):
        import contextlib
        import runpy
        from unittest.mock import Mock
        environment = {'BROWSER_AUTH_DIR': str(auth.DIRECTORY)}
        for command in ['initialize', 'encrypt-bootstrap', 'show-bootstrap', 'bootstrap-json', 'encrypt-current-bootstrap']:
            with self.subTest(command=command), patch.dict(os.environ, environment), patch.object(auth.sys, 'argv', [SPEC.origin, command]), patch.object(auth.sys, 'stdin', io.StringIO('{"username":"alice"}')), contextlib.redirect_stdout(io.StringIO()) as output:
                runpy.run_path(SPEC.origin, run_name='__main__')
            if command == 'encrypt-bootstrap':
                self.assertEqual(auth.decode(json.loads(output.getvalue())['username']), 'alice')
            if command == 'bootstrap-json':
                self.assertEqual(json.loads(output.getvalue())['username'], 'alice')
        bad = auth.DIRECTORY / 'invalid-bootstrap'
        with patch.dict(os.environ, {'BROWSER_AUTH_DIR': str(bad), 'BROWSER_PASSWORD': 'short'}), patch.object(auth.sys, 'argv', [SPEC.origin, 'initialize']):
            with self.assertRaises(SystemExit) as failure:
                runpy.run_path(SPEC.origin, run_name='__main__')
            self.assertIn('could not start', str(failure.exception))

    def test_server_entrypoint_always_stops_bridge(self):
        import runpy
        from unittest.mock import Mock
        server = Mock()
        # Use SystemExit to leave the real entrypoint's blocking server loop.
        server.serve_forever.side_effect = SystemExit(0)
        process = Mock(pid=123456)
        import http.server
        with patch.dict(os.environ, {'BROWSER_AUTH_DIR': str(auth.DIRECTORY)}), patch.object(auth.sys, 'argv', [SPEC.origin]), patch.object(http.server, 'ThreadingHTTPServer', return_value=server), patch.object(auth.threading, 'Thread') as thread, patch.object(auth.signal, 'signal') as signal_handler, patch.object(auth.subprocess, 'Popen', return_value=process), patch.object(auth.os, 'killpg') as kill:
            with self.assertRaises(SystemExit):
                runpy.run_path(SPEC.origin, run_name='__main__')
            server.serve_forever.assert_called_once()
            kill.assert_called()
            thread.return_value.start.assert_called_once()
            with self.assertRaises(SystemExit):
                signal_handler.call_args.args[1](None, None)


class SecurityRegressionTests(AuthHelpers, unittest.TestCase):
    def test_atomic_credentials_do_not_follow_predictable_temporary_symlinks(self):
        victim = auth.DIRECTORY / 'victim'
        victim.write_bytes(b'keep')
        link = auth.DIRECTORY / 'account.tmp'
        link.symlink_to(victim)
        auth.atomic(auth.DIRECTORY / 'account.enc', b'replacement')
        self.assertEqual(victim.read_bytes(), b'keep')
        self.assertTrue(link.is_symlink())
        self.assertEqual((auth.DIRECTORY / 'account.enc').read_bytes(), b'replacement')
        self.assertEqual(list(auth.DIRECTORY.glob('.account.enc-*')), [])

    def test_failed_credential_publication_keeps_committed_data_and_cleans_temporary_files(self):
        original = (auth.DIRECTORY / 'account.enc').read_bytes()
        for operation in ['replace', 'fsync']:
            with self.subTest(operation=operation), patch.object(auth.os, operation, side_effect=OSError('simulated disk failure')):
                with self.assertRaises(OSError):
                    auth.atomic(auth.DIRECTORY / 'account.enc', b'new')
            self.assertEqual((auth.DIRECTORY / 'account.enc').read_bytes(), original)
            self.assertEqual(list(auth.DIRECTORY.glob('.account.enc-*')), [])

    def test_invalid_unicode_csrf_and_password_are_rejected_without_handler_crashes(self):
        token = self.login()
        self.assertEqual(self.request('/auth/logout', {}, token, '\u00e9')[0], 403)
        csrf = auth.SESSIONS[token]['csrf']
        change = {'username': 'alice', 'password': '\ud800' * 12, 'confirm_password': '\ud800' * 12, 'current_password': 'a-unique-test-password'}
        self.assertEqual(self.request('/auth/account', change, token, csrf)[0], 400)
        self.assertIn(token, auth.SESSIONS)
        self.assertTrue(auth.matches(auth.account(), 'alice', 'a-unique-test-password'))


if __name__ == '__main__':
    unittest.main()


class UploadTests(AuthHelpers, unittest.TestCase):
    def test_upload_requires_session_csrf_origin_and_bounded_binary_body(self):
        token = self.login()
        csrf = auth.SESSIONS[token]['csrf']
        def upload(**kw):
            args = dict(raw=b'file', token=token, csrf=csrf, content_type='application/octet-stream', upload_name='document.txt')
            args.update(kw)
            return self.request('/auth/upload', **args)
        self.assertEqual(upload(token=None)[0], 401)
        self.assertEqual(upload(csrf='wrong')[0], 403)
        self.assertEqual(upload(origin='https://elsewhere')[0], 403)
        self.assertEqual(upload(content_type='application/json')[0], 415)
        self.assertEqual(upload(length=auth.UPLOAD_LIMIT+1)[0], 413)
        self.assertEqual(upload(length='invalid')[0], 400)
        self.assertEqual(upload(length=5)[0], 400)
        self.assertEqual(upload(upload_name='../outside.txt')[0], 400)
        self.assertEqual(upload(upload_name='%FF')[0], 400)
        with patch.object(auth, 'save_upload', side_effect=OSError('disk full')):
            self.assertEqual(upload()[0], 500)
        status, result, _ = upload(raw=b'\x00binary\xff')
        self.assertEqual(status, 201)
        path = Path(result['path'])
        self.assertTrue(path.is_relative_to(auth.DIRECTORY / 'uploads'))
        self.assertEqual(path.read_bytes(), b'\x00binary\xff')
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(upload(raw=b'')[0], 201)

    def test_upload_names_collisions_quota_and_failed_publication_are_safe(self):
        for name in ['', '.', '..', 'a%2Fb', 'a%5Cb', 'a:b', '%00bad', 'x'*201]:
            with self.assertRaises(ValueError):
                auth.save_upload(name, b'body')
        first = auth.save_upload('notes.txt', b'first')
        second = auth.save_upload('notes.txt', b'second')
        self.assertNotEqual(first, second)
        self.assertEqual(first.read_bytes(), b'first')
        with patch.object(auth, 'UPLOAD_LIMIT', 1), self.assertRaises(ValueError):
            auth.save_upload('big', b'xx')
        with patch.object(auth, 'UPLOAD_QUOTA', 2), self.assertRaises(ValueError):
            auth.save_upload('full', b'x')
        before = set((auth.DIRECTORY / 'uploads').iterdir())
        with patch.object(auth, 'atomic', side_effect=OSError('failed')), self.assertRaises(OSError):
            auth.save_upload('failed.txt', b'failed')
        self.assertEqual(set((auth.DIRECTORY / 'uploads').iterdir()), before)
        self.assertEqual(first.read_bytes(), b'first')
