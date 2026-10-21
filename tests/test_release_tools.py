"""Release helpers operate on a temporary app and source SDK, never installed libs."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]

def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT / 'scripts' / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

credentials = load('web-credentials')
media = load('bundle-macos-media')

class CredentialToolTests(unittest.TestCase):
    def test_rewrite_is_private_and_does_not_print_secrets(self):
        with tempfile.TemporaryDirectory() as directory, contextlib.redirect_stdout(io.StringIO()) as output:
            root = Path(directory)
            (root / '.env').write_text("OTHER=keep\nSERVO_WEB_USER=old\n")
            values = {'SERVO_WEB_USER': 'enc:v1:user', 'SERVO_WEB_PASSWORD': 'enc:v1:secret'}
            with patch.object(credentials.subprocess, 'check_output', return_value=json.dumps(values)) as run:
                credentials.main(['encrypt-env'], root)
                self.assertEqual(run.call_args.kwargs['cwd'], root)
            content = (root / '.env').read_text()
            self.assertIn('OTHER=keep', content)
            self.assertIn("SERVO_WEB_USER='enc:v1:user'", content)
            self.assertIn("SERVO_WEB_PASSWORD='enc:v1:secret'", content)
            self.assertNotIn('secret', output.getvalue())
            self.assertEqual((root / '.env').stat().st_mode & 0o777, 0o600)
        with patch.object(credentials.subprocess, 'check_output', return_value='explicit bootstrap output'), contextlib.redirect_stdout(io.StringIO()) as output:
            credentials.main(['show'])
            self.assertEqual(output.getvalue(), 'explicit bootstrap output')
        with patch.object(sys, 'argv', ['web-credentials.py']), self.assertRaises(SystemExit):
            runpy.run_path(credentials.__file__, run_name='__main__')

class MediaPackagerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.sdk = self.root / 'sdk/lib/gstreamer-1.0'
        self.sdk.mkdir(parents=True)
        self.app = self.root / 'Lotse.app'
        self.binary = self.app / 'Contents/MacOS/servo-browser'
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b'original executable')
        lists = self.root / 'vendor/servo/gstreamer_plugin_lists'
        lists.mkdir(parents=True)
        (lists / 'common.rs.in').write_text('["alpha"] // comment\n')
        (lists / 'macos.rs.in').write_text('["beta"]\n')
        for name in ['libalpha.dylib', 'libbeta.dylib', 'libhelper.dylib']:
            (self.sdk / name).write_bytes(b'source ' + name.encode())
        (self.sdk.parent / 'libcommon.dylib').write_bytes(b'common source')
        self.dependencies = {}
        self.calls = []

    def run_bundle(self, override=True):
        def check_output(command, **_):
            if command[0] == 'pkg-config':
                self.assertFalse(override)
                return str(self.sdk)
            self.assertEqual(command[0], 'otool')
            original = command[-1]
            deps = self.dependencies.get(original, [])
            return original + ':\n' + ''.join('  ' + dependency + ' (compatibility version 1.0.0)\n' for dependency in deps)
        with patch.dict(os.environ, {'SERVO_GSTREAMER_PLUGIN_DIR': str(self.sdk)} if override else {}, clear=True), patch.object(media.subprocess, 'check_output', side_effect=check_output), patch.object(media.subprocess, 'run', side_effect=lambda command, **_: self.calls.append(command)), contextlib.redirect_stdout(io.StringIO()):
            media.bundle(self.app, self.root)

    def test_dependency_closure_is_copied_rewritten_and_signed_only_inside_app(self):
        alpha = str(self.sdk / 'libalpha.dylib')
        beta = str(self.sdk / 'libbeta.dylib')
        common = str(self.sdk.parent / 'libcommon.dylib')
        self.dependencies[alpha] = [alpha, '/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation', '/usr/lib/libSystem.B.dylib', '@loader_path/libalpha.dylib', '@loader_path/libhelper.dylib', '@rpath/libcommon.dylib']
        self.dependencies[beta] = [common]
        self.dependencies[str(self.binary)] = [common]
        self.run_bundle(override=False)
        self.assertEqual((self.sdk / 'libalpha.dylib').read_bytes(), b'source libalpha.dylib')
        self.assertEqual(self.binary.read_bytes(), b'original executable')
        manifest = json.loads((self.app / 'Contents/Resources/media-libraries.json').read_text())
        self.assertEqual(len(manifest), 4)
        self.assertTrue(any('@loader_path/lib/libcommon.dylib' in command for command in self.calls))
        self.assertTrue(any(command[:2] == ['codesign', '--verify'] for command in self.calls))
        for command in self.calls:
            self.assertTrue(Path(command[-1]).is_relative_to(self.app))
        self.run_bundle()  # Override does not invoke pkg-config; existing copied files are safely replaced.

    def test_missing_collision_and_unresolved_dependencies_fail_before_signing(self):
        other = self.root / 'other/libalpha.dylib'
        other.parent.mkdir()
        other.write_bytes(b'other source')
        for dependency, message in [(str(other), 'collision'), ('@rpath/missing.dylib', 'Unresolved rpath'), ('relative.dylib', 'Unresolved dependency')]:
            self.dependencies[str(self.sdk / 'libalpha.dylib')] = [dependency]
            with self.subTest(dependency=dependency), self.assertRaisesRegex(RuntimeError, message):
                self.run_bundle()
        (self.sdk / 'libalpha.dylib').unlink()
        with self.assertRaisesRegex(RuntimeError, 'Missing media dependency'):
            self.run_bundle()
        with patch.object(sys, 'argv', ['bundle-macos-media.py']), self.assertRaises(SystemExit):
            runpy.run_path(media.__file__, run_name='__main__')
        with patch.object(media, 'bundle') as bundle:
            media.main([str(self.app)])
            bundle.assert_called_once_with(str(self.app))

if __name__ == '__main__':
    unittest.main()
