#!/usr/bin/env python3
"""Copy the selected GStreamer plugins and Mach-O dependency closure into our app.

Only copies from the SDK/Homebrew; never edits or re-signs installed libraries.
"""
from collections import deque
from pathlib import Path
import ast
import json
import os
import re
import shutil
import subprocess
import sys

def bundle(app, root=None):
    root=Path(root) if root is not None else Path(__file__).resolve().parents[1]
    app=Path(app).resolve()
    binary=app/'Contents/MacOS/servo-browser'
    output=binary.parent/'lib'
    output.mkdir(parents=True,exist_ok=True)
    override=os.environ.get('SERVO_GSTREAMER_PLUGIN_DIR')
    plugin_dir=Path(override or subprocess.check_output(['pkg-config','--variable=pluginsdir','gstreamer-1.0'],text=True).strip())
    plugins=[]
    for name in ('common','macos'):
        content=(root/f'vendor/servo/gstreamer_plugin_lists/{name}.rs.in').read_text()
        plugins.extend(ast.literal_eval(re.sub(r'//[^\n]*','',content)))
    queue=deque()
    seen={}
    manifest=[]
    def add(source):
        source=Path(source).resolve()
        if not source.is_file():raise RuntimeError(f'Missing media dependency: {source}')
        name=source.name
        if name in seen:
            if seen[name]!=source:raise RuntimeError(f'Library name collision: {name}')
            return output/name
        seen[name]=source
        destination=output/name
        temporary=destination.with_name(destination.name+'.new')
        shutil.copy2(source,temporary)
        temporary.chmod(0o755)
        temporary.replace(destination)
        queue.append((source,destination))
        manifest.append({'file':name,'source':str(source)})
        return destination
    for plugin in plugins:add(plugin_dir/f'lib{plugin}.dylib')
    queue.append((binary,binary))
    while queue:
        original,destination=queue.popleft()
        dependencies=subprocess.check_output(['otool','-L',str(original)],text=True).splitlines()[1:]
        edits=[]
        for entry in dependencies:
            dependency=entry.strip().split(' (',1)[0]
            if dependency==str(original):continue
            if dependency.startswith(('/System/','/usr/lib/')):continue
            path=Path(dependency)
            if dependency.startswith('@loader_path/'):
                path=original.parent/dependency.removeprefix('@loader_path/')
            elif dependency.startswith('@rpath/'):
                candidate=plugin_dir.parent/dependency.removeprefix('@rpath/')
                if not candidate.exists():raise RuntimeError(f'Unresolved rpath {dependency} in {original}')
                path=candidate
            elif not path.is_absolute():raise RuntimeError(f'Unresolved dependency {dependency} in {original}')
            if path.resolve()==original.resolve():continue
            copied=add(path)
            target=('@loader_path/lib/' if destination==binary else '@loader_path/')+copied.name
            edits.extend(['-change',dependency,target])
        if destination!=binary:
            edits[0:0]=['-id','@rpath/'+destination.name]
        if edits:subprocess.run(['install_name_tool',*edits,str(destination)],check=True)
    for library in output.glob('*.dylib'):
        subprocess.run(['codesign','--force','--sign','-','--timestamp=none',str(library)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    # Rewriting dependencies invalidates the executable's existing ad-hoc signature.
    (app/'Contents/Resources').mkdir(exist_ok=True)
    (app/'Contents/Resources/media-libraries.json').write_text(json.dumps(manifest,indent=2))
    # Seal the complete bundle only after every resource has been written.
    subprocess.run(['codesign','--force','--sign','-','--timestamp=none',str(app)],check=True)
    subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
    print(f'Bundled {len(plugins)} GStreamer plugins and {len(seen)} native libraries.')


def main(args=None):
    args = sys.argv[1:] if args is None else args
    if len(args) != 1:
        raise SystemExit('Usage: scripts/bundle-macos-media.py APP')
    bundle(args[0])


if __name__ == '__main__':
    main()
