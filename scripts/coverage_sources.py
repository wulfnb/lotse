"""Bind Rust coverage to the exact production sources and build configuration."""
import hashlib
from pathlib import Path


def fingerprint(root):
    root = Path(root)
    digest = hashlib.sha256()
    files = set(root.glob('src/**/*.rs')) | set(root.glob('src/**/*.js'))
    files |= {root / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')}
    files |= {path for path in root.glob('engines/**/*') if path.is_file() and 'target' not in path.parts and path.suffix in {'.rs','.js','.toml','.lock'}}
    # Vendored code generators and allocator fixes affect the tested engine even
    # when the dependency version/path in Cargo.lock stays the same.
    files |= {path for path in root.glob('vendor/**/*')
              if path.is_file() and not {'target', '__pycache__'}.intersection(path.relative_to(root).parts)
              and (path.suffix in {'.rs', '.py', '.toml', '.webidl', '.in', '.glsl'} or path.name == 'Bindings.conf')}
    files = sorted(files)
    for path in files:
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b'\0')
        digest.update(path.read_bytes())
        digest.update(b'\0')
    return digest.hexdigest()
