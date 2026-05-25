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
LOG_DIRS = ['/tmp', '/var/log']
ARENA_PATH = '/dev/shm'

AGENT_ROLES = ['queen', 'worker', 'drone', 'honeybee', 'weaver', 'swarm']

# ── Data ──────────────────────────────────────────────────────────────────────

def get_state():
    agents = []
    for role in AGENT_ROLES:
        try:
            r = subprocess.run(['pgrep', '-f', role], capture_output=True, text=True, timeout=2)
            for pid in r.stdout.strip().split():
                if pid:
                    mem = _proc_mem(pid)
                    agents.append({'pid': int(pid), 'role': role, 'mem': mem})
        except Exception:
            pass

    shm = [f.name for f in Path(ARENA_PATH).glob('hive_*')] if Path(ARENA_PATH).exists() else []

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

def get_agent_reasoning():
    """Read agent logs and extract reasoning."""
    reasoning = {}
    for agent in AGENT_ROLES:
        lines = []
        for ld in LOG_DIRS:
            logfile = Path(ld) / f'hive_{agent}.log'
            if logfile.exists():
                try:
                    with open(logfile) as f:
                        all_lines = f.readlines()
                        lines = [l.strip() for l in all_lines[-15:] if l.strip()]
                except Exception:
                    pass
                break
        if lines:
            reasoning[agent] = lines
    return reasoning

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
    padding:20px;max-width:1200px;margin:0 auto;
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
.reasoning-panel{background:var(--surface);border:1px solid var(--border);border-radius:4px;padding:10px 14px;margin-bottom:10px}
.reasoning-panel .agent-header{font-size:11px;font-weight:700;letter-spacing:1px;margin-bottom:6px;display:flex;align-items:center;gap:8px}
.reasoning-panel .log-line{font-size:10px;color:var(--text2);padding:1px 0;line-height:1.4;word-break:break-all}
.reasoning-panel .log-line:hover{color:var(--text)}
.reasoning-panel .line-time{color:var(--text2);opacity:0.5;margin-right:6px}
.footer{color:var(--text2);font-size:10px;margin-top:20px;text-align:center;border-top:1px solid var(--border);padding-top:12px}
.hive-banner{font-size:10px;color:var(--text2);text-align:center;margin-bottom:10px;opacity:0.6}
"""

ROLE_ICONS = {'worker':'◈','drone':'◆','honeybee':'◉','weaver':'✦','queen':'◇','swarm':'⬡'}
ROLE_COLORS = {'worker':'green','drone':'cyan','honeybee':'orange','weaver':'purple','queen':'gold','swarm':'red'}

# ── HTML ──────────────────────────────────────────────────────────────────────

PAGE = """<!DOCTYPE html>
<html><head><meta charset="UTF-8"><title>HIVE C2</title>
<meta http-equiv="refresh" content="5">
<style>""" + THEME + """</style></head><body>
<pre>""" + HIVE_LOGO + """</pre>
<div class="hive-banner">""" + HIVE_BANNER + """</div>
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
        <div class="metric-extra">$agent_list</div>
    </div>
    <div class="card">
        <div class="value">$shm_n</div>
        <div class="label">SHARED MEM FILES</div>
        <div class="metric-extra">/dev/shm/hive_*</div>
    </div>
    <div class="card">
        <div class="value">$memfd_n</div>
        <div class="label">MEMFD (FILELESS)</div>
        <div class="metric-extra">anonymous pages</div>
    </div>
    <div class="card">
        <div class="value">${total_mem}M</div>
        <div class="label">TOTAL MEMORY</div>
        <div class="metric-extra">${avg_mem}M avg per agent</div>
    </div>
</div>
<h2>◈ AGENT REGISTRY</h2>
<table>
<tr><th>PID</th><th>ROLE</th><th>MEMORY</th><th>STATUS</th></tr>
$agent_rows
</table>
<h2>✦ AGENT REASONING (last 15 lines)</h2>
$reasoning_html
<p class="footer">$timestamp · HIVE C2 v3.0 · auto-refresh 5s · <a href="/api/state" style="color:var(--cyan)">API</a> · <a href="/api/reasoning" style="color:var(--cyan)">REASONING</a></p>
</body></html>"""

# ── Handler ───────────────────────────────────────────────────────────────────

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args): pass

    def do_GET(self):
        if self.path == '/api/state':
            self._json(get_state())
        elif self.path == '/api/reasoning':
            self._json(get_agent_reasoning())
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

        # Status
        if agents_n == 0:
            dot_class, status_text, tag, tag_class = 'idle', 'AWAITING HIVE', 'DORMANT', 'warn'
        elif agents_n >= 3:
            dot_class, status_text, tag, tag_class = 'live', 'HIVE ACTIVE', 'OPERATIONAL', 'ok'
        else:
            dot_class, status_text, tag, tag_class = 'live', 'BOOTSTRAPPING', 'PARTIAL', 'warn'

        # Agent rows
        role_order = ['queen','worker','drone','honeybee','weaver','swarm']
        rows = ''
        agent_names = []
        for a in sorted(s['agents'], key=lambda x: role_order.index(x['role']) if x['role'] in role_order else 99):
            icon = ROLE_ICONS.get(a['role'], '?')
            color = ROLE_COLORS.get(a['role'], 'green')
            agent_names.append(a['role'])
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

        # Reasoning panel
        reasoning = get_agent_reasoning()
        reasoning_html = ''
        for role in role_order:
            icon = ROLE_ICONS.get(role, '?')
            color = ROLE_COLORS.get(role, 'green')
            lines = reasoning.get(role, [])
            if not lines and not any(a['role'] == role for a in s['agents']):
                continue
            # If agent is running but no log file, note it
            is_running = any(a['role'] == role for a in s['agents'])
            header = f'<span class="{color}">{icon}</span> {role.upper()}'
            if not is_running:
                header += ' <span style="color:var(--red);font-size:9px">(offline)</span>'

            if lines:
                log_html = ''
                for line in lines[-15:]:
                    # Try to extract timestamp from structured log
                    ts = ''
                    rest = line
                    if line.startswith('\x1b['):
                        # ANSI escape — skip
                        pass
                    log_html += f'<div class="log-line">{line[:200]}</div>'
            else:
                log_html = '<div class="log-line" style="color:var(--text2);opacity:0.4">(no log data)</div>'

            reasoning_html += f'''
            <div class="reasoning-panel">
                <div class="agent-header">{header}</div>
                {log_html}
            </div>'''

        if not reasoning_html:
            reasoning_html = '<div class="reasoning-panel"><div class="log-line" style="color:var(--text2);opacity:0.4;text-align:center;padding:10px">awaiting agent reasoning...</div></div>'

        avg_mem = total_mem // max(agents_n, 1)
        agent_list = ', '.join(agent_names) if agent_names else '—'

        html = string.Template(PAGE).substitute(
            dot_class=dot_class, status_text=status_text, tag=tag, tag_class=tag_class,
            uptime=str(s['uptime']),
            agents_n=str(agents_n), shm_n=str(len(s['shm'])),
            memfd_n=str(s['memfds']), total_mem=str(total_mem),
            avg_mem=str(avg_mem),
            agent_rows=rows, timestamp=s['timestamp'],
            agent_list=agent_list,
            reasoning_html=reasoning_html,
        )
        self.send_response(200)
        self.send_header('Content-Type', 'text/html; charset=utf-8')
        self.end_headers()
        self.wfile.write(html.encode())


class Server(socketserver.ThreadingMixIn, http.server.HTTPServer):
    allow_reuse_address = True
    daemon_threads = True

def main():
    import argparse
    p = argparse.ArgumentParser(description='HIVE C2 Dashboard')
    p.add_argument("--port", type=int, default=8080, help="HTTP port")
    p.add_argument('--host', default='0.0.0.0')
    args = p.parse_args()

    srv = Server((args.host, args.port), Handler)
    print(f'╔══════════════════════════════════════╗')
    print(f'║   HIVE C2 DASHBOARD v3.0           ║')
    print(f'║   http://{args.host}:{args.port}                 ║')
    print(f'║   API: /api/state                   ║')
    print(f'║   API: /api/reasoning               ║')
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
