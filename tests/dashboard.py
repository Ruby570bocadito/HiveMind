#!/usr/bin/env python3
"""HIVE - Multi-Agent C2 Dashboard. Cyberpunk theme."""

import http.server
import json
import os
import socketserver
import string
import subprocess
import time
from datetime import datetime
from pathlib import Path

BORN = time.time()

# ── Data ──────────────────────────────────────────────────────────────────────

def get_state():
    agents = []
    for role in ['worker', 'drone', 'weaver', 'honeybee', 'queen', 'swarm']:
        try:
            r = subprocess.run(['pgrep', '-f', role], capture_output=True, text=True, timeout=2)
            for pid in r.stdout.strip().split():
                if pid:
                    mem = _proc_mem(pid)
                    agents.append({'pid': int(pid), 'role': role, 'mem': mem})
        except Exception:
            pass

    shm = [f.name for f in Path('/dev/shm').glob('hive_*')] if Path('/dev/shm').exists() else []

    memfds = 0
    try:
        for d in Path('/proc').glob('[0-9]*/fd/*'):
            try:
                lnk = os.readlink(str(d))
                if 'memfd:hive' in lnk or 'memfd:scout' in lnk or 'memfd:shaper' in lnk:
                    memfds += 1
            except Exception:
                pass
    except Exception:
        pass

    return {
        'timestamp': datetime.now().strftime('%H:%M:%S'),
        'uptime': int(time.time() - BORN),
        'agents': agents,
        'shm': shm,
        'memfds': memfds,
    }

def _proc_mem(pid):
    try:
        with open(f'/proc/{pid}/status') as f:
            for line in f:
                if line.startswith('VmRSS:'):
                    return int(line.split()[1]) // 1024  # MB
    except Exception:
        return 0

# ── ASCII Art ─────────────────────────────────────────────────────────────────

HIVE_LOGO = r"""
 __         .' '.
        _/__)        .   .       .
       (8|)_}}- .      .        .
jgs     `\__)    '. . ' ' .  . '
    H I V E   C O L O N Y
"""

HIVE_BANNER = """
  .________________________________________________________________.
  |  WORKER ◈  DRONE ◆  HONEYBEE ◉  WEAVER ✦  QUEEN ◇  SWARM ⬡  |
  '----------------------------------------------------------------'
"""

# ── Theme ─────────────────────────────────────────────────────────────────────

THEME = """
:root {
    --bg: #090d0f;
    --surface: #0d1518;
    --surface2: #111d22;
    --border: #1a2d35;
    --text: #a8c5d6;
    --text2: #6d8a9e;
    --green: #3aeb8c;
    --green2: #1a7a4a;
    --red: #ff4060;
    --cyan: #20e0e0;
    --gold: #e0b040;
    --purple: #a080ff;
    --orange: #ff8030;
    --glow: 0 0 12px;
}
*{margin:0;padding:0;box-sizing:border-box}
body{
    background:var(--bg);color:var(--text);
    font:13px/1.5 'JetBrains Mono','Fira Code','Cascadia Code',monospace;
    padding:20px;max-width:1000px;margin:0 auto;
    min-height:100vh;
}
pre{color:var(--green);font-size:10px;line-height:1;margin:0 0 12px 0;text-shadow:0 0 4px rgba(58,235,140,0.3)}
h1{color:var(--cyan);font-size:14px;font-weight:400;margin:0 0 16px 0;letter-spacing:2px;text-transform:uppercase}
h2{color:var(--cyan);font-size:12px;font-weight:400;margin:20px 0 8px;letter-spacing:1px}
.status-bar{
    display:flex;gap:12px;margin:0 0 20px 0;align-items:center;
    background:var(--surface);border:1px solid var(--border);border-radius:4px;padding:10px 16px;
    font-size:11px;
}
.status-bar .dot{width:8px;height:8px;border-radius:50%;display:inline-block;margin-right:6px}
.dot-live{background:var(--green);box-shadow:0 0 6px var(--green);animation:pulse 1.5s infinite}
.dot-dead{background:var(--red)}
.dot-idle{background:var(--gold)}
@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.4}}
.spacer{flex:1}
.tag{padding:2px 8px;border-radius:3px;font-size:10px;font-weight:700;letter-spacing:1px}
.tag-ok{background:rgba(58,235,140,0.12);color:var(--green);border:1px solid var(--green2)}
.tag-warn{background:rgba(224,176,64,0.12);color:var(--gold);border:1px solid rgba(224,176,64,0.3)}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:10px}
.card{
    background:var(--surface);border:1px solid var(--border);border-radius:4px;
    padding:14px 16px;transition:border-color 0.3s
}
.card:hover{border-color:var(--green2)}
.card .value{font-size:32px;font-weight:700;color:var(--cyan);text-shadow:0 0 8px rgba(32,224,224,0.2)}
.card .label{font-size:10px;color:var(--text2);text-transform:uppercase;letter-spacing:1px;margin-top:2px}
.metric-extra{font-size:10px;color:var(--text2);margin-top:4px}
table{width:100%;border-collapse:collapse;font-size:12px}
th{text-align:left;color:var(--text2);padding:5px 10px;border-bottom:1px solid var(--border);font-weight:400;font-size:10px;text-transform:uppercase;letter-spacing:1px}
td{padding:5px 10px;border-bottom:1px solid var(--border)}
.green{color:var(--green)}.cyan{color:var(--cyan)}.red{color:var(--red)}.gold{color:var(--gold)}.purple{color:var(--purple)}.orange{color:var(--orange)}
.role-icon{display:inline-block;width:14px;text-align:center;margin-right:6px;font-size:10px}
.agent-row:hover{background:var(--surface2)}
.footer{color:var(--text2);font-size:10px;margin-top:20px;text-align:center;border-top:1px solid var(--border);padding-top:12px}
"""

ROLE_ICONS = {'worker':'◈','drone':'◆','honeybee':'◉','weaver':'✦','queen':'◇','swarm':'⬡'}
ROLE_COLORS = {'worker':'green','drone':'cyan','honeybee':'orange','weaver':'purple','queen':'gold','swarm':'red'}

# ── HTML ──────────────────────────────────────────────────────────────────────

PAGE = """<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>HIVE C2</title>
<meta http-equiv="refresh" content="3">
<style>""" + THEME + """</style></head><body>
<pre>""" + HIVE_LOGO + """</pre>
<div class="status-bar">
    <span class="dot dot-$dot_class"></span>
    <span>$status_text</span>
    <span class="spacer"></span>
    <span class="tag tag-$tag_class">$tag</span>
    <span class="spacer"></span>
    <span style="color:var(--text2)">uptime ${uptime}s</span>
</div>
<div class="grid">
    <div class="card">
        <div class="value">$agents_n</div>
        <div class="label">AGENTS ACTIVE</div>
    </div>
    <div class="card">
        <div class="value">$shm_n</div>
        <div class="label">SHARED MEM FILES</div>
    </div>
    <div class="card">
        <div class="value">$memfd_n</div>
        <div class="label">MEMFD (FILELESS)</div>
    </div>
    <div class="card">
        <div class="value">${total_mem}M</div>
        <div class="label">TOTAL MEMORY</div>
    </div>
</div>
<h2>◈ AGENT REGISTRY</h2>
<table>
<tr><th>PID</th><th>ROLE</th><th>MEMORY</th><th>STATUS</th></tr>
$agent_rows
</table>
<p class="footer">$timestamp · HIVE C2 v2.0 · auto-refresh 3s</p>
</body></html>"""

# ── Handler ───────────────────────────────────────────────────────────────────

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args): pass

    def do_GET(self):
        if self.path == '/api/state':
            self._json(get_state())
        else:
            self._html()

    def _json(self, data):
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data, indent=2).encode())

    def _html(self):
        s = get_state()
        agents_n = len(s['agents'])
        total_mem = sum(a['mem'] for a in s['agents'])

        # Status logic
        if agents_n == 0:
            dot_class, status_text, tag, tag_class = 'idle', 'AWAITING HIVE', 'DORMANT', 'warn'
        elif agents_n >= 3:
            dot_class, status_text, tag, tag_class = 'live', 'HIVE ACTIVE', 'OPERATIONAL', 'ok'
        else:
            dot_class, status_text, tag, tag_class = 'live', 'BOOTSTRAPPING', 'PARTIAL', 'warn'

        # Agent rows
        rows = ''
        for a in sorted(s['agents'], key=lambda x: ['worker','drone','weaver','honeybee','queen','swarm'].index(x['role']) if x['role'] in ['worker','drone','weaver','honeybee','queen','swarm'] else 99):
            icon = ROLE_ICONS.get(a['role'], '?')
            color = ROLE_COLORS.get(a['role'], 'green')
            rows += (
                f'<tr class="agent-row">'
                f'<td style="color:var(--text2)">{a["pid"]}</td>'
                f'<td><span class="role-icon {color}">{icon}</span><span class="{color}">{a["role"].upper()}</span></td>'
                f'<td style="color:var(--text2)">{a["mem"]} MB</td>'
                f'<td><span class="green">● ACTIVE</span></td>'
                f'</tr>'
            )
        if not rows:
            rows = '<tr><td colspan="4" style="color:var(--text2);text-align:center;padding:20px">no agents in hive memory</td></tr>'

        html = string.Template(PAGE).substitute(
            dot_class=dot_class, status_text=status_text, tag=tag, tag_class=tag_class,
            uptime=str(s['uptime']),
            agents_n=str(agents_n), shm_n=str(len(s['shm'])),
            memfd_n=str(s['memfds']), total_mem=str(total_mem),
            agent_rows=rows, timestamp=s['timestamp'],
        )
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(html.encode())


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    allow_reuse_address = True
    _obfuscated_port: int = 8080  # Rotates periodically
    daemon_threads = True

def main():
    import argparse
    p = argparse.ArgumentParser(description='HIVE C2 Dashboard')
    p.add_argument("--port", type=int, default=None, help="HTTP port (random if not set)")
    p.add_argument('--host', default='127.0.0.1')
    args = p.parse_args()
    if args.port is None:
        import random
        args.port = random.randint(8000, 9000)
        print(f"  [OBFUSCATED] Using random port: {args.port}")

    srv = Server((args.host, args.port), Handler)
    print(f'╔══════════════════════════════════════╗')
    print(f'║   HIVE C2 DASHBOARD v2.0           ║')
    print(f'║   http://{args.host}:{args.port}                 ║')
    print(f'║   API: /api/state                   ║')
    print(f'║   Ctrl+C to stop                    ║')
    print(f'╚══════════════════════════════════════╝')
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        srv.shutdown()
        srv.server_close()
        print('\n╚══ HIVE DASHBOARD TERMINATED ══╝')

if __name__ == '__main__':
    main()
