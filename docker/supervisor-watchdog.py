#!/usr/bin/env python3
"""Supervisor event listener for container-level recovery and maintenance.

PROCESS_STATE_FATAL: a program exhausted its start retries. Supervisor would
leave it dead while the container keeps running, so stop supervisord instead;
the container's restart policy (`restart: unless-stopped`) then starts a fresh
container.

TICK_3600: renew the generated TLS certificate when needed (or re-copy a
user-supplied one) and reload nginx if the certificate changed.

Protocol: http://supervisord.org/events.html (stdout carries only protocol
tokens; diagnostics go to stderr).
"""
import hashlib
import os
from pathlib import Path
import signal
import subprocess
import sys

CERTIFICATE = Path('/config/tls/server.crt')
NGINX_PID = Path('/run/wega/browser/nginx.pid')
RENEW = ['/usr/local/bin/entrypoint.sh', 'renew-tls']


def fields(line):
    return dict(token.split(':', 1) for token in line.split())


def log(message):
    print('watchdog: ' + message, file=sys.stderr, flush=True)


def fingerprint():
    try:
        return hashlib.sha256(CERTIFICATE.read_bytes()).hexdigest()
    except OSError:
        return None


def renew_tls():
    before = fingerprint()
    result = subprocess.run(RENEW, stdin=subprocess.DEVNULL, stdout=sys.stderr, stderr=sys.stderr, check=False)
    if result.returncode:
        log(f'certificate renewal check failed with status {result.returncode}')
    if fingerprint() != before:
        try:
            os.kill(int(NGINX_PID.read_text().strip()), signal.SIGHUP)
            log('TLS certificate changed; nginx reloaded')
        except (OSError, ValueError) as error:
            log(f'TLS certificate changed but nginx could not be reloaded: {error}')


def handle(header, payload, supervisor):
    event = header.get('eventname', '')
    if event == 'PROCESS_STATE_FATAL':
        name = fields(payload.split('\n', 1)[0]).get('processname', 'unknown')
        log(f'{name} entered FATAL; stopping supervisord so the container restart policy restarts Wega')
        os.kill(supervisor, signal.SIGTERM)
    elif event.startswith('TICK_'):
        renew_tls()


def main(stdin=sys.stdin, stdout=sys.stdout, supervisor=None):
    supervisor = os.getppid() if supervisor is None else supervisor
    while True:
        stdout.write('READY\n')
        stdout.flush()
        line = stdin.readline()
        if not line:
            return
        header = fields(line)
        payload = stdin.read(int(header.get('len', '0')))
        try:
            handle(header, payload, supervisor)
        except Exception as error:  # Never break the protocol; supervisor would stall events.
            log(f'event handling failed: {error!r}')
        stdout.write('RESULT 2\nOK')
        stdout.flush()


if __name__ == '__main__':
    main()
