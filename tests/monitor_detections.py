#!/usr/bin/env python3
"""
Hive Colony EDR Detection Monitor.
Simula la superficie de detección que un EDR real (CrowdStrike, Defender, SentinelOne)
vería al monitorizar la actividad de Hive.

Chequea:
  1. Puertos TCP listening (deberían ser 0 para Hive)
  2. Memoria compartida (/dev/shm colmena_*, hive_*)
  3. Firmas de procesos (worker, drone, honeybee, etc.)
  4. Archivos xattr (stigmergy)
  5. Mutación de datos (saboteur)
  6. Fragmentos de genoma (phoenix)
  7. Cápsulas chrononaut
  8. Conexiones de red
  9. Modificaciones de timestamps
"""

import os, re, sys, time, json, argparse, subprocess
from datetime import datetime
from pathlib import Path

DETECTION_RULES = {
    "tcp_listener": {
        "severity": "HIGH",
        "description": "Puerto TCP listening (Hive no debería tener ninguno)",
        "check": lambda: any(check_port(p) for p in [1337, 31337, 4242, 5555, 8443, 9000, 11434]),
    },
    "shared_memory_hive": {
        "severity": "LOW",
        "description": "Memoria compartida Hive en /dev/shm",
        "check": lambda: len(list(Path("/dev/shm").glob("hive_*"))) > 0 if Path("/dev/shm").exists() else False,
    },
    "hive_processes": {
        "severity": "MEDIUM",
        "description": "Procesos Hive en ejecución (worker/drone/honeybee/queen/weaver)",
        "check": check_hive_processes,
    },
    "saboteur_data_mutation": {
        "severity": "HIGH",
        "description": "Mutación de datos financieros/CSV/JSON (Saboteur)",
        "check": check_data_integrity,
    },
    "phoenix_genome_fragments": {
        "severity": "MEDIUM",
        "description": "Fragmentos de genoma ocultos (Phoenix en /dev/shm/.hive*)",
        "check": lambda: any(
            Path(p).exists() for p in ["/dev/shm/.hive_genome", "/tmp/.hive_rebirth", "/var/tmp/.hs_"]
        ),
    },
    "stigmergy_xattr": {
        "severity": "LOW",
        "description": "Atributos extendidos xattr (Stigmergy en /bin/ls, /bin/ps)",
        "check": check_stigmergy_xattr,
    },
    "chrononaut_timestamp_anomaly": {
        "severity": "MEDIUM",
        "description": "Anomalías en timestamps de archivos (Chrononaut capsules)",
        "check": check_timestamp_anomalies,
    },
    "whispernet_p2p": {
        "severity": "MEDIUM",
        "description": "Tráfico P2P entre agentes (WhisperNet mesh)",
        "check": check_whispernet_traffic,
    },
    "binary_on_disk": {
        "severity": "MEDIUM",
        "description": "Binarios Hive en disco fuera de /tmp",
        "check": check_agent_binaries,
    },
    "fileless_memfd": {
        "severity": "HIGH",
        "description": "MemFDs activos (ejecución fileless vía stinger)",
        "check": lambda: len([f for f in Path("/proc").glob("*/fd/*") if os.path.islink(str(f)) and "memfd:" in os.readlink(str(f))]) > 0 if Path("/proc").exists() else False,
    },
}

def check_port(port):
    try:
        import socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(0.5)
        r = s.connect_ex(('127.0.0.1', port))
        s.close()
        return r == 0
    except: return False

def check_hive_processes():
    try:
        r = subprocess.run(["ps", "aux"], capture_output=True, text=True, timeout=5)
        agents = ['worker', 'drone', 'honeybee', 'queen', 'weaver', 'stinger', 'beekeeper']
        return sum(1 for line in r.stdout.split('\n') if any(a in line.lower() for a in agents))
    except: return 0

def check_data_integrity():
    """Busca mutaciones de Saboteur en CSVs."""
    findings = 0
    for csv in Path("/tmp").rglob("*.csv"):
        try:
            data = csv.read_text()
            # Saboteur inserta filas corruptas o modifica balances
            if "nan" in data.lower() or "null" in data.lower() or "ERROR" in data:
                findings += 1
        except: pass
    for json_f in Path("/tmp").rglob("*.json"):
        try:
            data = json_f.read_text()
            if "corrupted" in data.lower() or "TAMPERED" in data:
                findings += 1
        except: pass
    return findings > 0

def check_stigmergy_xattr():
    try:
        for bin_path in ["/bin/ls", "/bin/ps", "/usr/bin/ssh", "/bin/bash"]:
            r = subprocess.run(["getfattr", "-d", "-m", "user.hive", bin_path],
                             capture_output=True, text=True, timeout=3)
            if "user.hive" in r.stdout:
                return True
    except: pass
    return False

def check_timestamp_anomalies():
    """Chrononaut codifica timestamps futuros en mtime."""
    anomalies = 0
    now = time.time()
    try:
        for log in Path("/var/log").rglob("*.log") if Path("/var/log").exists() else []:
            mtime = log.stat().st_mtime
            if mtime > now + 3600 or mtime < 1000000000:
                anomalies += 1
    except: pass
    return anomalies > 0

def check_whispernet_traffic():
    try:
        r = subprocess.run(["ss", "-tnp"], capture_output=True, text=True, timeout=3)
        # WhisperNet usa puertos altos no estándar
        suspicious = 0
        for line in r.stdout.split('\n'):
            if any(p in line for p in [":900", ":10000", ":20000"]):
                suspicious += 1
        return suspicious > 0
    except: return False

def check_agent_binaries():
    try:
        r = subprocess.run(["find", "/tmp", "-name", "hive_*", "-o", "-name", ".*hive*"],
                         capture_output=True, text=True, timeout=3)
        return len(r.stdout.strip().split('\n')) > 1 if r.stdout.strip() else False
    except: return False


class EDRMonitor:
    def __init__(self, watch=False, interval=5, json_output=False):
        self.watch = watch
        self.interval = interval
        self.json_output = json_output
        self.alerts = []

    def scan(self):
        timestamp = datetime.now().isoformat()
        findings = {}
        for name, rule in DETECTION_RULES.items():
            try:
                detected = rule['check']()
                findings[name] = {'detected': bool(detected), 'severity': rule['severity'],
                                  'description': rule['description']}
                if detected:
                    self.alerts.append({'timestamp': timestamp, 'rule': name,
                                        'severity': rule['severity'], 'description': rule['description']})
            except Exception as e:
                findings[name] = {'detected': None, 'severity': rule['severity'],
                                  'description': f"Error: {e}"}
        return findings

    def report(self, findings):
        if self.json_output:
            print(json.dumps({'timestamp': self.alerts[-1]['timestamp'] if self.alerts else datetime.now().isoformat(),
                             'findings': findings, 'alerts': self.alerts}, indent=2))
            return

        print(f"\n{'='*60}")
        print(f"  Hive Detection Monitor - {datetime.now().strftime('%H:%M:%S')}")
        print(f"{'='*60}")

        high = medium = low = 0
        for name, r in findings.items():
            d, s, desc = r['detected'], r['severity'], r['description']
            if d:
                icon = {"HIGH": "CRITICAL", "MEDIUM": "WARNING", "LOW": "INFO"}.get(s, "INFO")
                {"HIGH": high, "MEDIUM": medium, "LOW": low}[s] += 1
                print(f"  [{icon:>8}] {desc}")
            elif d is None:
                print(f"  [  ERROR ] {desc}")

        if high + medium + low == 0:
            print(f"\n  [+] CLEAN — Sin superficie de detección expuesta")
        else:
            score = max(0, 100 - (high * 30 + medium * 15 + low * 5))
            print(f"\n  Detection Score: {score}/100")
            print(f"  Alertas: {high} HIGH, {medium} MEDIUM, {low} LOW")

    def run(self):
        print("HIVE EDR DETECTION MONITOR")
        print("Monitoreando indicadores detectables por EDR...")
        while True:
            f = self.scan()
            self.report(f)
            if not self.watch: break
            time.sleep(self.interval)


def main():
    p = argparse.ArgumentParser(description='Hive EDR Detection Monitor')
    p.add_argument('--watch', action='store_true', help='Monitoreo continuo')
    p.add_argument('--interval', type=int, default=5, help='Intervalo (segundos)')
    p.add_argument('--json', action='store_true', help='Salida JSON')
    args = p.parse_args()
    EDRMonitor(watch=args.watch, interval=args.interval, json_output=args.json).run()

if __name__ == '__main__':
    main()
