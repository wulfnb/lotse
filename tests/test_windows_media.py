"""Real PE import-table parsing and selective SDK dependency closure contracts."""
from contextlib import redirect_stdout
import importlib.util
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('windows_media',ROOT/'scripts/bundle-windows-media.py')
media=importlib.util.module_from_spec(spec);spec.loader.exec_module(media)

def pe(names=(),delay=()):
    data=bytearray(4096);data[:2]=b'MZ';struct.pack_into('<I',data,0x3c,0x80);data[0x80:0x84]=b'PE\0\0'
    struct.pack_into('<HH',data,0x84,0x8664,1);struct.pack_into('<H',data,0x94,0xf0)
    optional=0x98;struct.pack_into('<H',data,optional,0x20b);struct.pack_into('<Q',data,optional+24,0x140000000);struct.pack_into('<I',data,optional+60,512);struct.pack_into('<I',data,optional+108,16)
    struct.pack_into('<IIII',data,optional+0xf0+8,3584,0x1000,3584,512)
    name_at=0x900
    for index,items,size,base,name_offset in [(1,names,20,0x200,12),(13,delay,32,0x500,4)]:
        if not items:continue
        struct.pack_into('<II',data,optional+112+8*index,base+0xe00,(len(items)+1)*size)
        for n,name in enumerate(items):
            row=base+n*size
            if index==13:struct.pack_into('<I',data,row,1)
            struct.pack_into('<I',data,row+name_offset,name_at+0xe00)
            value=name.encode()+b'\0';data[name_at:name_at+len(value)]=value;name_at+=len(value)
    return bytes(data)

class WindowsMediaTests(unittest.TestCase):
    def test_normal_and_delayed_imports_and_architecture_guards(self):
        with tempfile.TemporaryDirectory() as directory:
            file=Path(directory)/'test.dll';file.write_bytes(pe(['KERNEL32.dll','glib.dll'],['delay.dll']))
            self.assertEqual(media.imports(file),['delay.dll','glib.dll','KERNEL32.dll'])
            data=bytearray(file.read_bytes());struct.pack_into('<H',data,0x84,0x14c);file.write_bytes(data)
            with self.assertRaisesRegex(ValueError,'x64'):media.imports(file)
            file.write_bytes(b'not PE')
            with self.assertRaisesRegex(ValueError,'Not a PE'):media.imports(file)

    def test_exact_selected_closure_includes_licenses_but_excludes_unused_codecs(self):
        with tempfile.TemporaryDirectory() as directory,redirect_stdout(io.StringIO()):
            root=Path(directory);sdk=root/'sdk';dest=root/'app';system=root/'system'
            for path in [sdk/'bin',sdk/'lib/gstreamer-1.0',sdk/'share/licenses',dest,system]:path.mkdir(parents=True,exist_ok=True)
            (sdk/'WEGA-SDK-PROVENANCE.json').write_text(json.dumps({'version':media.SDK_VERSION,'installer_sha256':media.SDK_SHA256,'source':'official SDK fixture'}))
            (sdk/'share/licenses/LGPL.txt').write_text('License fixture')
            (system/'KERNEL32.dll').write_bytes(b'OS DLL')
            (system/'glib.dll').write_bytes(b'Developer-installed copy must not hide SDK dependency')
            (dest/'wega.exe').write_bytes(pe(['gstreamer-1.0-0.dll','libEGL.dll']))
            (dest/'libEGL.dll').write_bytes(pe(['KERNEL32.dll']))
            for name in media.plugins():(sdk/'lib/gstreamer-1.0'/name).write_bytes(pe(['KERNEL32.dll']))
            (sdk/'bin/gstreamer-1.0-0.dll').write_bytes(pe(['glib.dll'],['delay.dll']))
            for name in ['glib.dll','delay.dll','avcodec.dll']:(sdk/'bin'/name).write_bytes(pe())
            media.bundle(sdk,dest,system)
            manifest=json.loads((dest/'MEDIA-MANIFEST.json').read_text())
            self.assertTrue((dest/'glib.dll').exists());self.assertTrue((dest/'delay.dll').exists())
            self.assertFalse((dest/'avcodec.dll').exists());self.assertFalse((dest/'KERNEL32.dll').exists())
            self.assertEqual(len(manifest['files']),len(media.plugins())+3)
            self.assertTrue((dest/'media-licenses/LGPL.txt').exists())
            (dest/'wega.exe').write_bytes(pe(['avcodec.dll']))
            with self.assertRaisesRegex(ValueError,'Excluded codec'):media.bundle(sdk,dest,system)
            (system/'VCRUNTIME140.dll').write_bytes(b'Developer-installed VC runtime')
            (dest/'wega.exe').write_bytes(pe(['VCRUNTIME140.dll']))
            with self.assertRaisesRegex(ValueError,'Missing runtime'):media.bundle(sdk,dest,system)
            runtime=root/'redist/v143/x64/Microsoft.VC143.CRT';runtime.mkdir(parents=True)
            (runtime/'VCRUNTIME140.dll').write_bytes(pe())
            media.bundle(sdk,dest,system,runtime)
            self.assertTrue((dest/'VCRUNTIME140.dll').exists())
            self.assertTrue((dest/'MICROSOFT-VC-RUNTIME-NOTICE.txt').exists())
            records=json.loads((dest/'MEDIA-MANIFEST.json').read_text())['files']
            self.assertEqual(next(item for item in records if item['file']=='VCRUNTIME140.dll')['source'],'Microsoft Visual C++ REDIST')
            (dest/'wega.exe').write_bytes(pe(['missing.dll']))
            with self.assertRaisesRegex(ValueError,'Missing runtime'):media.bundle(sdk,dest,system)

if __name__=='__main__':unittest.main()
