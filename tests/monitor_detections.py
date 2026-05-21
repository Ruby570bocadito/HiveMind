#!/usr/bin/env python3
"""
EDR Sandbox Monitor - Simulates detection surface monitoring.
Checks for suspicious indicators that an EDR would flag.

Usage:
    python3 monitor_detections.py [--watch] [--interval 5]

Checks:
    1. TCP listening ports (should be NONE for the colmena)
    2. Suspicious shared memory names
    3. Process signatures (running agents)
    4. ONNX model signatures in binaries
    5. Network connections
"""

import os
import re
import sys
import time
import json
import argparse
import subprocess
from datetime import datetime
from pathlib import Path

# ── Detection Rules ──────────────────────────────────────────────────────────

DETECTION_RULES = {
    "tcp_listener_4242": {
        "severity": "HIGH",
        "description": "Colmena TCP bus listening (legacy port 4242)",
        "check": lambda: check_port_listening(4242),
    },
    "tcp_listener_suspicious": {
        "severity": "MEDIUM",
        "description": "Suspicious TCP listener on high port",
        "check": lambda: any(check_port_listening(p) for p in [1337, 31337, 5555, 9000]),
    },
    "shared_memory_named": {
        "severity": "LOW",
        "description": "Named shared memory (colmena_*) in /dev/shm",
        "check": check_shared_memory,
    },
    "onnx_signature_in_memory": {
        "severity": "MEDIUM",
        "description": "ONNX model signatures in process memory",
        "check": check_onnx_in_memory,
    },
    "process_injection": {
        "severity": "HIGH",
        "description": "Suspicious process creation patterns",
        "check": check_process_patterns,
    },
    "binary_on_disk": {
        "severity": "MEDIUM",
        "description": "Agent binaries found on disk outside temp",
        "check": check_agent_binaries,
    },
    "network_connections": {
        "severity": "MEDIUM",
        "description": "Outbound network connections from agents",
        "check": check_network_connections,
    },
}

# ── Check Implementations ────────────────────────────────────────────────────

def check_port_listening(port):
    """Check if a TCP port is listening."""
    try:
        import socket
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(0.5)
        result = s.connect_ex(('127.0.0.1', port))
        s.close()
        return result == 0
    except Exception:
        return False


def check_shared_memory():
    """Check for colmena-related shared memory files."""
    shm_dir = Path("/dev/shm")
    if not shm_dir.exists():
        return False
    colmena_files = list(shm_dir.glob("colmena_*"))
    return len(colmena_files) > 0


def check_onnx_in_memory():
    """Check process maps for ONNX signatures."""
    try:
        result = subprocess.run(
            ["grep", "-r", "--include=*.onnx", "/proc/*/maps"],
            capture_output=True, text=True, timeout=5
        )
        return result.returncode == 0
    except Exception:
        return False


def check_process_patterns():
    """Check for suspicious process creation."""
    suspicious = []
    try:
        result = subprocess.run(
            ["ps", "aux"],
            capture_output=True, text=True, timeout=5
        )
        for line in result.stdout.split('\n'):
            if any(name in line.lower() for name in ['worker', 'drone', 'honeybee', 'weaver']):
                suspicious.append(line.strip())
    except Exception:
        pass
    return len(suspicious) > 0


def check_agent_binaries():
    """Check for agent binaries outside temp directories."""
    agent_names = ['worker', 'drone', 'honeybee', 'weaver', 'queen', 'colmena_bus']
    found = []
    try:
        result = subprocess.run(
            ["find", "/tmp", "-name", "*scout*", "-o", "-name", "*shaper*",
             "-o", "-name", "*weaver*", "-o", "-name", "*hoarder*"],
            capture_output=True, text=True, timeout=5
        )
        for line in result.stdout.split('\n'):
            if line.strip():
                found.append(line.strip())
    except Exception:
        pass
    return len(found) > 0


def check_network_connections():
    """Check for outbound connections from agent processes."""
    try:
        result = subprocess.run(
            ["ss", "-tnp"],
            capture_output=True, text=True, timeout=5
        )
        for line in result.stdout.split('\n'):
            # Check for connections to non-standard ports from our agents
            if '127.0.0.1:4242' in line:
                return True
    except Exception:
        pass
    return False


# ── Monitor ──────────────────────────────────────────────────────────────────

class EDRMonitor:
    def __init__(self, watch=False, interval=5):
        self.watch = watch
        self.interval = interval
        self.history = []
        self.alerts = []

    def scan(self):
        """Run all detection rules and report findings."""
        timestamp = datetime.now().isoformat()
        findings = {}

        for rule_name, rule in DETECTION_RULES.items():
            try:
                detected = rule['check']()
                findings[rule_name] = {
                    'detected': detected,
                    'severity': rule['severity'],
                    'description': rule['description'],
                }
                if detected:
                    self.alerts.append({
                        'timestamp': timestamp,
                        'rule': rule_name,
                        'severity': rule['severity'],
                        'description': rule['description'],
                    })
            except Exception as e:
                findings[rule_name] = {
                    'detected': None,
                    'severity': rule['severity'],
                    'description': f"Error: {e}",
                }

        self.history.append({
            'timestamp': timestamp,
            'findings': findings,
        })

        return findings

    def report(self, findings):
        """Print a formatted report."""
        print(f"\n{'='*60}")
        print(f"  EDR Detection Surface Scan - {datetime.now().strftime('%H:%M:%S')}")
        print(f"{'='*60}")

        high = medium = low = 0
        for rule_name, result in findings.items():
            detected = result['detected']
            severity = result['severity']
            desc = result['description']

            if detected is True:
                if severity == 'HIGH':
                    high += 1
                    icon = "CRITICAL"
                elif severity == 'MEDIUM':
                    medium += 1
                    icon = "WARNING"
                else:
                    low += 1
                    icon = "INFO"
                print(f"  [{icon}] {desc}")
            elif detected is False:
                pass  # Clean
            else:
                print(f"  [ERROR] {desc}")

        if high + medium + low == 0:
            print(f"\n  [+] CLEAN - No detection surface exposed")
        else:
            total = high + medium + low
            score = max(0, 100 - (high * 30 + medium * 15 + low * 5))
            print(f"\n  Detection Score: {score}/100")
            print(f"  Alerts: {high} high, {medium} medium, {low} low")

        return high + medium + low

    def run(self):
        """Run monitoring loop."""
        print("COLMENA EDR DETECTION MONITOR")
        print("Monitoring for indicators that an EDR would flag...")
        print("(Run the colmena first, then this monitor)")
        print()

        while True:
            findings = self.scan()
            alerts = self.report(findings)

            if not self.watch:
                break

            time.sleep(self.interval)


# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description='Colmena EDR Detection Monitor')
    parser.add_argument('--watch', action='store_true', help='Continuous monitoring')
    parser.add_argument('--interval', type=int, default=5, help='Scan interval (seconds)')
    parser.add_argument('--json', action='store_true', help='Output as JSON')
    args = parser.parse_args()

    monitor = EDRMonitor(watch=args.watch, interval=args.interval)

    if args.json:
        findings = monitor.scan()
        print(json.dumps({
            'timestamp': monitor.history[-1]['timestamp'],
            'findings': findings,
        }, indent=2))
    else:
        monitor.run()


if __name__ == '__main__':
    main()
