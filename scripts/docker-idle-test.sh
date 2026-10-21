#!/bin/sh
# Catch background-tab redraw loops while the live status counters are enabled.
set -eu
cd "$(dirname "$0")/.."
docker compose exec -T servo-browser mkdir -p /tmp/servo-idle-fixtures
docker compose cp tests/fixtures/. servo-browser:/tmp/servo-idle-fixtures
docker compose exec -T --user browser servo-browser python3 - <<'PY'
import json, os, pathlib, subprocess, tempfile, time
profile = pathlib.Path(tempfile.mkdtemp(prefix='servo-idle.'))
(profile / 'browser.json').write_text(json.dumps({
    'urls': ['file:///tmp/servo-idle-fixtures/interaction.html',
             'file:///tmp/servo-idle-fixtures/second.html'],
    'active': 1,
}))
env = dict(os.environ, DISPLAY=':96', BROWSER_METRICS_SCOPE='process')
with (profile / 'display.log').open('w') as display_log, (profile / 'browser.log').open('w') as log:
    display = subprocess.Popen(['Xvfb', ':96', '-screen', '0', '1440x900x24', '-nolisten', 'tcp', '-ac'], stdout=display_log, stderr=display_log)
    browser = None
    try:
        for attempt in range(50):
            if subprocess.run(['xdpyinfo'], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                break
            time.sleep(.2)
        else:
            raise RuntimeError('Idle-test display did not start')
        browser = subprocess.Popen(['servo-browser', '--profile-dir', str(profile)], env=env, stdout=log, stderr=log)
        time.sleep(8)
        def ticks():
            assert browser.poll() is None, 'Browser exited before idle measurement'
            fields = pathlib.Path(f'/proc/{browser.pid}/stat').read_text().rsplit(') ', 1)[1].split()
            return int(fields[11]) + int(fields[12])
        before, started = ticks(), time.monotonic()
        time.sleep(3)
        cpu = (ticks() - before) / os.sysconf('SC_CLK_TCK') / (time.monotonic() - started) * 100
        assert cpu < 100, f'Idle redraw loop: {cpu:.1f}% CPU on two static tabs'
        print(f'IDLE TEST PASSED: {cpu:.1f}% CPU with two static tabs and live resource counters (100% = one core)')
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
