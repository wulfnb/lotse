"""Deterministic range/attachment/auth fixture for native download integration."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import sys

BODY = bytes(i % 251 for i in range(6 * 1024 * 1024 + 173))
class Handler(BaseHTTPRequestHandler):
    def do_HEAD(self):
        self.respond(True)
    def do_GET(self):
        self.respond(False)
    def respond(self, head):
        if self.path == '/':
            body = b'<!doctype html><title>Download fixture</title><h1>Downloads</h1><a id="download" href="/file.bin" download="fixture.bin">Download multipart fixture</a>'
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Set-Cookie', 'download-auth=ok; HttpOnly; Path=/')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            if not head: self.wfile.write(body)
            return
        if self.path != '/file.bin':
            self.send_error(404); return
        if 'download-auth=ok' not in self.headers.get('Cookie', ''):
            self.send_error(401); return
        start, end = 0, len(BODY) - 1
        ranged = self.headers.get('Range', '').startswith('bytes=')
        if ranged:
            try:
                start, end = map(int, self.headers['Range'][6:].split('-'))
                assert 0 <= start <= end < len(BODY)
            except (ValueError, AssertionError):
                self.send_error(416); return
        self.send_response(206 if ranged else 200)
        self.send_header('Content-Type', 'application/octet-stream')
        self.send_header('Content-Disposition', 'attachment; filename="fixture.bin"')
        self.send_header('Accept-Ranges', 'bytes')
        self.send_header('ETag', '"fixture-v1"')
        self.send_header('Content-Length', str(end - start + 1))
        if ranged: self.send_header('Content-Range', f'bytes {start}-{end}/{len(BODY)}')
        self.end_headers()
        if not head:
            try: self.wfile.write(BODY[start:end+1])
            except (BrokenPipeError, ConnectionResetError): pass

if __name__ == '__main__':
    ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1]) if len(sys.argv) > 1 else 8771), Handler).serve_forever()
