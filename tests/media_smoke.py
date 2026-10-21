"""Drive real muted audio/video decoding through the browser's W3C HTTP endpoint."""
import base64
import json
import os
from pathlib import Path
import subprocess
import socket
import sys
import tempfile
import time
import urllib.request
import urllib.error

binary = str(Path(sys.argv[1] if len(sys.argv) > 1 else 'target/debug/servo-browser').resolve())
profile = Path(tempfile.mkdtemp(prefix='servo-media-', dir=os.environ.get('LOTSE_TEST_OUTPUT')))
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    port = listener.getsockname()[1]
base = f'http://127.0.0.1:{port}'
def request(method,path,body=None):
    data = None if body is None else json.dumps(body).encode()
    req=urllib.request.Request(base+path,data=data,method=method,headers={'Content-Type':'application/json'})
    try:
        with urllib.request.urlopen(req,timeout=36) as response:
            return json.load(response)['value']
    except urllib.error.HTTPError as error:
        raise AssertionError(error.read().decode()) from error
with (profile/'browser.log').open('w') as log:
    process=subprocess.Popen([binary,'--profile-dir',str(profile),'--webdriver-port',str(port),'servo://newtab'],stdout=log,stderr=log)
    try:
        deadline=time.monotonic()+30
        while True:
            try:
                if request('GET','/status')['ready']:
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError('Browser did not become ready')
                time.sleep(.2)
            except OSError:
                if time.monotonic()>deadline or process.poll() is not None:raise RuntimeError((profile/'browser.log').read_text())
                time.sleep(.2)
        session=request('POST','/session',{'capabilities':{'alwaysMatch':{'browserName':'servo'}}})['sessionId']
        root=f'/session/{session}'
        request('POST',root+'/url',{'url':os.environ.get('LOTSE_FIXTURE_URL', 'http://127.0.0.1:8767') + '/media.html'})
        deadline=time.monotonic()+25
        while True:
            result=request('POST',root+'/execute/sync',{'script':'return window.mediaReport','args':[]})
            assert not result.get('videoError') and not result.get('audioError'),result
            if result.get('videoTime',0)>.5 and result.get('audioTime',0)>.5 and result.get('videoWidth')==320:break
            if time.monotonic()>deadline:raise AssertionError(result)
            time.sleep(.2)
        (profile/'media-report.json').write_text(json.dumps(result,indent=2))
        request('POST',root+'/execute/sync',{'script':'video.pause();audio.pause();if(navigator.mediaSession)navigator.mediaSession.playbackState="paused";return true','args':[]})
        (profile/'media-playback.png').write_bytes(base64.b64decode(request('GET',root+'/screenshot')))
        request('DELETE',root)
        print(f'MEDIA TEST PASSED: actual WebM video and WAV audio decode, advancing playback clocks and 320×180 video frames. Muted throughout. Artifacts: {profile}',flush=True)
    finally:
        process.terminate()
        try:process.wait(timeout=10)
        except subprocess.TimeoutExpired:process.kill();process.wait()
