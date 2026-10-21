"""Bind Rust coverage to the exact production sources and build configuration."""
import hashlib
from pathlib import Path


def fingerprint(root):
    root = Path(root)
    digest = hashlib.sha256()
    files = sorted(set(root.glob('src/**/*.rs')) | {root / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')})
    for path in files:
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b'\0')
        digest.update(path.read_bytes())
        digest.update(b'\0')
    return digest.hexdigest()
