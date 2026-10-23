#!/usr/bin/env python3
"""Check required web auth and host-mounted downloads without logging credentials."""
import base64
import http.client
import json
from pathlib import Path
import ssl
import subprocess
import uuid

ROOT = Path(__file__).resolve().parent.parent


def docker(*args):
    return subprocess.check_output(["docker", *args], cwd=ROOT, text=True)


container = docker("compose", "ps", "-q", "wega").strip()
info = json.loads(docker("inspect", container))[0]
environment = dict(item.split("=", 1) for item in info["Config"]["Env"])
credentials = json.loads(docker("exec", container, "python3", "/opt/wega/auth-service.py", "bootstrap-json"))
port = int(info["NetworkSettings"]["Ports"]["7443/tcp"][0]["HostPort"])
host = f"localhost:{port}"
context = ssl._create_unverified_context()  # Local generated certificate.


def request(path, cookie=None, websocket=False, data=None):
    headers = {}
    if cookie:
        headers["Cookie"] = cookie
    if data is not None:
        headers.update({"Content-Type": "application/json", "Origin": f"https://{host}"})
    if websocket:
        headers.update({
            "Origin": f"https://{host}", "Connection": "Upgrade",
            "Upgrade": "websocket", "Sec-WebSocket-Version": "13",
            "Sec-WebSocket-Key": base64.b64encode(uuid.uuid4().bytes).decode(),
            "Sec-WebSocket-Protocol": "binary",
        })
    connection = http.client.HTTPSConnection("localhost", port, context=context, timeout=10)
    try:
        connection.request("POST" if data is not None else "GET", path, body=json.dumps(data) if data is not None else None, headers=headers)
        response = connection.getresponse()
        assert "no-store" in response.getheader("Cache-Control", ""), f"Response may be cached: {path}"
        return response.status, b"" if websocket else response.read(), response.getheader("Set-Cookie")
    finally:
        connection.close()


filename = "wega-auth-check-" + uuid.uuid4().hex + ".txt"
payload = "Authenticated download mount check\n"
try:
    docker("exec", "--user", "browser", container, "python3", "-c",
           "import pathlib,sys; p=pathlib.Path('/config/Downloads')/sys.argv[1]; "
           "p.open('x').write(sys.argv[2])", filename, payload)
    assert b'login-form' in request("/")[1]
    assert request("/auth/login", data={"username": "invalid", "password": "invalid"})[0] == 401
    status, _, cookie_header = request("/auth/login", data=credentials)
    assert status == 200, "Login failed (bootstrap credentials may have changed in Account)"
    assert all(flag in cookie_header for flag in ("Secure", "HttpOnly", "SameSite=Strict"))
    cookie = cookie_header.split(";", 1)[0]
    for path in ("/index.html", "/viewer.js", "/novnc/core/rfb.js", "/downloads/", f"/downloads/{filename}"):
        assert request(path)[0] == 401, f"Anonymous access allowed: {path}"
        assert request(path, "__Host-wega_session=invalid")[0] == 401
        assert request(path, cookie)[0] == 200, f"Session access failed: {path}"
    assert request("/healthz")[:2] == (200, b"ok\n")
    assert request("/websockify", websocket=True)[0] == 401
    assert request("/websockify", cookie, websocket=True)[0] == 101
    assert request(f"/downloads/{filename}", cookie)[1] == payload.encode()
    if environment.get("WEGA_HTTP_PORT"):
        # The plain-HTTP port is reached the way a TLS-terminating proxy would: from
        # inside the network, forwarding the public Host. Only https:// Origins pass.
        http_check = f"""
import http.client, json, sys
def call(path, origin, data=None, cookie=None, websocket=False):
    headers = {{"Host": "wega.example", "Origin": origin}}
    if cookie: headers["Cookie"] = cookie
    if data is not None: headers["Content-Type"] = "application/json"
    if websocket: headers.update({{"Connection": "Upgrade", "Upgrade": "websocket", "Sec-WebSocket-Version": "13",
                                  "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==", "Sec-WebSocket-Protocol": "binary"}})
    connection = http.client.HTTPConnection("127.0.0.1", {int(environment["WEGA_HTTP_PORT"])}, timeout=10)
    connection.request("POST" if data is not None else "GET", path, body=None if data is None else json.dumps(data), headers=headers)
    response = connection.getresponse()
    return response.status, response.getheader("Set-Cookie")
credentials = json.load(sys.stdin)
assert call("/auth/login", "http://wega.example", credentials)[0] == 403
status, cookie = call("/auth/login", "https://wega.example", credentials)
assert status == 200 and "Secure" in cookie and cookie.startswith("__Host-")
cookie = cookie.split(";", 1)[0]
assert call("/novnc/core/rfb.js", "https://wega.example")[0] == 401
assert call("/novnc/core/rfb.js", "https://wega.example", cookie=cookie)[0] == 200
assert call("/websockify", "http://wega.example", cookie=cookie, websocket=True)[0] == 403
assert call("/websockify", "https://wega.example", cookie=cookie, websocket=True)[0] == 101
"""
        subprocess.run(["docker", "exec", "-i", container, "python3", "-c", http_check],
                       input=json.dumps(credentials), text=True, check=True, cwd=ROOT)
        print("PASS: plain-HTTP port accepts only https:// origins (login, assets, WebSocket)")
    mount = next(m for m in info["Mounts"] if m["Destination"] == "/config/Downloads")
    assert mount["Type"] == "bind", "Downloads must use the configured host directory"
    assert (Path(mount["Source"]) / filename).read_text() == payload
    print("PASS: required login, rejected invalid credentials, authenticated viewer/assets/"
          "WebSocket/downloads, public health check, and configured host download directory")
finally:
    docker("exec", "--user", "browser", container, "rm", "-f", "/config/Downloads/" + filename)
