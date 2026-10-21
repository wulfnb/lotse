# Authentication security fixes — 21 October 2026

These focused fixes preserve the existing authentication, encrypted credentials,
CSRF, session revocation and secure-cookie behavior. They do not certify Servo
or the browser as a whole for security-sensitive use.

## Exclusive credential publication

The old credential writer used a predictable `account.tmp` path with `O_TRUNC`.
An existing symlink at that path could cause the write to truncate its target
before publication. The writer now uses `tempfile.mkstemp` to create an exclusive
private inode in the destination directory, synchronizes its contents, replaces
the destination and cleans up temporary files on failures.

Regression tests create the old predictable symlink and verify its target remains
unchanged. They also simulate failed synchronization and replacement, verifying
that committed credentials survive and temporary files are removed. These tests
use temporary directories and disposable credentials.

## Malformed Unicode input

String-based `hmac.compare_digest` rejects non-ASCII strings with a TypeError.
CSRF headers are now compared as UTF-8 bytes so malformed tokens return 403.
Passwords containing unpaired surrogates from JSON are rejected with a validation
error rather than crashing during UTF-8 encoding. Tests verify rejection preserves
existing credentials and sessions.

## Validation

The full Python suite passed 28 tests. All 249 executable gateway lines were
covered. Existing account rotation, session expiration/logout, WebSocket teardown,
rate limiting, origin checks and encrypted credential tests still pass. Full
Docker deployment and upstream engine security are separate verification scopes.

## Release gating

The release workflow now calls the CI workflow before its packaging jobs. Required
CI failures block all release builds and publishing, rather than allowing a main
branch push to publish independently of test and coverage results. Separate CI
concurrency groups prevent the called workflow from canceling standalone checks.
The workflow configuration was checked locally; remote CI and releases were not
run, and the current coverage target intentionally keeps release gates failing.

Release-tag input is passed as environment data and checked with
`git check-ref-format`, preventing shell/output injection through interpolated
workflow input. Local checks verify newline injection is rejected. Checksum
creation excludes its own output; local checks verify every listed asset hash,
including a filename beginning with a dash. Windows packaging now builds with the
desktop feature and fails if the executable is missing. These are configuration
fixes; Windows binaries and remote release jobs were not built in this session.
All workflow files passed `actionlint` (including ShellCheck).
