"""Fail-closed noVNC integration and served stylesheet regressions."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('novnc_patch',ROOT/'docker/patch-novnc.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)

class ViewerPatchTests(unittest.TestCase):
    def test_refuses_unaudited_source_without_writing_either_file(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'core').mkdir()
            for name in ['display.js','rfb.js']:(root/'core'/name).write_text('different upstream source')
            with self.assertRaisesRegex(ValueError,'Unaudited'):module.patch(root)
            self.assertEqual((root/'core/display.js').read_text(),'different upstream source')

    def test_missing_anchor_is_atomic_even_with_approved_source_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);(root/'core').mkdir()
            display='\n'.join(['        this._renderQ = [];','    // ===== PUBLIC METHODS =====','        this._renderQ.push(action);','                this._targetCtx.drawImage(this._backbuffer,'])
            (root/'core/display.js').write_text(display);(root/'core/rfb.js').write_text('missing integration points')
            audited={name:hashlib.sha256((root/'core'/name).read_bytes()).hexdigest() for name in ['display.js','rfb.js']}
            with patch.object(module,'AUDITED',audited),self.assertRaisesRegex(ValueError,'Unsupported'):module.patch(root)
            self.assertEqual((root/'core/display.js').read_text(),display)

    def test_viewer_css_is_a_stylesheet_with_usable_touch_controls(self):
        css=(ROOT/'docker/viewer/viewer.css').read_text()
        self.assertNotIn('<html',css);self.assertNotIn('<!doctype',css)
        self.assertIn('touch-action:none',css);self.assertIn('44px',css)

if __name__=='__main__':unittest.main()
