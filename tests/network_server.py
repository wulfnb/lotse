"""Deterministic HTTP endpoints for the native developer-tool integration test."""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from pathlib import Path
import json
import sys
import time

class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(Path(__file__).parent / 'fixtures'), **kwargs)
    def end_headers(self):
        if self.path.split('?', 1)[0] == '/interaction.html':
            self.send_header('Cache-Control', 'no-store')
        if self.path.split('?', 1)[0] == '/toolbox.html':
            self.send_header('Set-Cookie', 'lotse_visible=visible-fixture; Path=/; SameSite=Lax')
            self.send_header('Set-Cookie', 'lotse_http_only=native-only-fixture; Path=/; HttpOnly; SameSite=Strict; Expires=Wed, 21 Oct 2037 07:28:00 GMT')
            self.send_header('Set-Cookie', 'lotse_wrong_path=not-for-page; Path=/different-cookie-scope; HttpOnly')
        super().end_headers()
    def send_json(self, status, data):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('X-Servo-Fixture', 'network-test')
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        if self.path.split('?', 1)[0] == '/interaction.html':
            visits = getattr(self.server, 'interaction_visits', 0)
            self.server.interaction_visits = visits + 1
            if visits:
                time.sleep(getattr(self.server, 'smoke_reload_delay', 0))
        if self.path.startswith('/auth-fixture-'):
            if self.headers.get('Authorization') == 'Basic Zml4dHVyZTp0ZXN0LXBhc3M=':
                self.send_json(200, {'authenticated': True})
            else:
                self.send_response(401)
                self.send_header('WWW-Authenticate', 'Basic realm="' + self.path.strip('/') + '"')
                self.send_header('Content-Length', '0')
                self.end_headers()
            return
        if self.path.startswith('/api/data'):
            time.sleep(0.12)
            self.send_json(200, {'message': 'real network response', 'items': [1, 2, 3]})
        elif self.path == '/api/missing':
            self.send_json(404, {'error': 'fixture missing'})
        elif self.path == '/api/redirect':
            self.send_response(302)
            self.send_header('Location', '/api/data?redirected=1')
            self.send_header('Content-Length', '0')
            self.end_headers()
        else:
            super().do_GET()
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        self.send_json(201, {'received': body.decode(), 'method': 'POST'})

if __name__ == '__main__':
    ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1]) if len(sys.argv)>1 else 8767), Handler).serve_forever()
