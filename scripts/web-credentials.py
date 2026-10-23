#!/usr/bin/env python3
"""Local bootstrap-secret utilities. `show` intentionally displays initial credentials."""
import json
from pathlib import Path
import re
import subprocess
import sys

def main(args=None, root=None):
    root = Path(root) if root is not None else Path(__file__).resolve().parent.parent
    args = sys.argv[1:] if args is None else args
    operation = args[0] if args else ''
    if operation not in ('show', 'encrypt-env'):
        sys.exit('Usage: python3 scripts/web-credentials.py show|encrypt-env')
    command = 'show-bootstrap' if operation == 'show' else 'encrypt-current-bootstrap'
    result = subprocess.check_output(['docker', 'compose', 'exec', '-T', 'wega',
                                      'python3', '/opt/wega/auth-service.py', command], cwd=root, text=True)
    if operation == 'show':
        print(result, end='')
    else:
        path = root / '.env'
        content = path.read_text()
        for name, value in json.loads(result).items():
            replacement = name + "='" + value + "'"
            pattern = r'^' + re.escape(name) + r'=.*$'
            if re.search(pattern, content, re.MULTILINE):
                content = re.sub(pattern, lambda _: replacement, content, flags=re.MULTILINE)
            else:
                content += '\n' + replacement + '\n'
        path.write_text(content)
        path.chmod(0o600)
        print('Bootstrap username/password encrypted in .env; encryption key remains in the private profile volume.')


if __name__ == "__main__":
    main()
