#!/bin/sh
# Catch background-tab redraw loops while the live status counters are enabled.
#
# Threshold: two static tabs should cost a few percent of one core once settled
# (docs/docker.md records ~1.3% for a whole idle container). The regressions this guards against (redraw or
# repaint loops) peg llvmpipe at 50–100%+. 10% of one core over a 10 s window
# leaves ample margin for shared CI runners without letting a loop through.
# Whole-container CPU over the same window is reported, not asserted: it also
# includes the user's own session (browser, x11vnc polling) on the test host.
set -eu
cd "$(dirname "$0")/.."
docker compose exec -T wega mkdir -p /tmp/wega-idle-fixtures
# The image filesystem is read-only (docker cp cannot write to it); stream into /tmp (tmpfs).
COPYFILE_DISABLE=1 tar -C tests/fixtures -cf - . | docker compose exec -T wega tar -C /tmp/wega-idle-fixtures --no-same-owner -xf -
docker compose exec -T --user browser wega python3 - <<'PY'
import json, os, pathlib, subprocess, tempfile, time
SETTLE, WINDOW, LIMIT = 8, 10, 10.0
profile = pathlib.Path(tempfile.mkdtemp(prefix='wega-idle.'))
(profile / 'browser.json').write_text(json.dumps({
    'urls': ['file:///tmp/wega-idle-fixtures/interaction.html',
             'file:///tmp/wega-idle-fixtures/second.html'],
    'active': 1,
}))
# Same renderer settings as the supervised browser (docker/start-browser.sh).
env = dict(os.environ, DISPLAY=':96', BROWSER_METRICS_SCOPE='process',
           LP_NUM_THREADS=os.environ.get('WEGA_RENDER_THREADS', '2'),
           GALLIUM_DRIVER=os.environ.get('WEGA_GALLIUM_DRIVER', 'llvmpipe'))


def container_usec():
    try:
        for line in pathlib.Path('/sys/fs/cgroup/cpu.stat').read_text().splitlines():
            if line.startswith('usage_usec '):
                return int(line.split()[1])
    except OSError:
        pass
    return None


with (profile / 'display.log').open('w') as display_log, (profile / 'browser.log').open('w') as log:
    display = subprocess.Popen(['Xvfb', ':96', '-screen', '0', '1280x800x24', '-nolisten', 'tcp', '-ac'], stdout=display_log, stderr=display_log)
    browser = None
    try:
        for attempt in range(50):
            if subprocess.run(['xdpyinfo'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(.2)
        else:
            raise RuntimeError('Idle-test display did not start')
        browser = subprocess.Popen(['wega', '--profile-dir', str(profile)], env=env, stdout=log, stderr=log)
        time.sleep(SETTLE)
        def ticks():
            assert browser.poll() is None, 'Browser exited before idle measurement'
            fields = pathlib.Path(f'/proc/{browser.pid}/stat').read_text().rsplit(') ', 1)[1].split()
            return int(fields[11]) + int(fields[12])
        before, container_before, started = ticks(), container_usec(), time.monotonic()
        time.sleep(WINDOW)
        elapsed = time.monotonic() - started
        cpu = (ticks() - before) / os.sysconf('SC_CLK_TCK') / elapsed * 100
        container_after = container_usec()
        total = (f'{(container_after - container_before) / 1e6 / elapsed * 100:.1f}%'
                 if container_before is not None and container_after is not None else 'unavailable')
        assert cpu < LIMIT, f'Idle redraw loop: {cpu:.1f}% of one core on two static tabs (limit {LIMIT:.0f}%)'
        print(f'IDLE TEST PASSED: test browser {cpu:.1f}% of one core over {WINDOW}s after {SETTLE}s settle '
              f'(limit {LIMIT:.0f}%); whole container {total} (100% = one core)')
    finally:
        for process in (browser, display):
            if process is not None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
PY
