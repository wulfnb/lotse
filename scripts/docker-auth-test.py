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


container = docker("compose", "ps", "-q", "servo-browser").strip()
info = json.loads(docker("inspect", container))[0]
environment = dict(item.split("=", 1) for item in info["Config"]["Env"])
credentials = json.loads(docker("exec", container, "python3", "/opt/servo/auth-service.py", "bootstrap-json"))
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


filename = "servo-auth-check-" + uuid.uuid4().hex + ".txt"
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
        assert request(path, "__Host-servo_session=invalid")[0] == 401
        assert request(path, cookie)[0] == 200, f"Session access failed: {path}"
    assert request("/healthz")[:2] == (200, b"ok\n")
    assert request("/websockify", websocket=True)[0] == 401
    assert request("/websockify", cookie, websocket=True)[0] == 101
    assert request(f"/downloads/{filename}", cookie)[1] == payload.encode()
    mount = next(m for m in info["Mounts"] if m["Destination"] == "/config/Downloads")
    assert mount["Type"] == "bind", "Downloads must use the configured host directory"
    assert (Path(mount["Source"]) / filename).read_text() == payload
    print("PASS: required login, rejected invalid credentials, authenticated viewer/assets/"
          "WebSocket/downloads, public health check, and configured host download directory")
finally:
    docker("exec", "--user", "browser", container, "rm", "-f", "/config/Downloads/" + filename)
