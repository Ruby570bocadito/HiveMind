"""
Generate synthetic dataset for Scout agent classifier.
Creates labeled samples of process/system profiles with EDR indicators.
"""

import csv
import random
import os

NORMAL_PROCESSES = [
    "system", "csrss", "svchost", "explorer", "lsass", "services",
    "wininit", "winlogon", "smss", "taskhost", "dwm", "spoolsv",
    "audiodg", "fontdrvhost", "sihost", "runtimebroker", "searchui",
    "shell_experience", "startmenuexperiencehost", "textinputhost",
    "chrome", "firefox", "edge", "code", "teams", "outlook",
    "excel", "word", "powerpoint", "notepad", "calc",
]

EDR_PROCESSES = [
    "csfalcon", "csagent", "msmpeng", "sentinelagent",
    "carbonblack", "cylancesvc", "symantec", "mcafee_framework",
]

BACKUP_PROCESSES = [
    "veeam", "backup_exec", "commvault", "netbackup",
]

FEATURES = [
    "process_count", "network_connections", "cpu_usage", "memory_usage",
    "disk_io", "has_edr_process", "has_backup_process", "admin_privileges",
    "domain_joined", "firewall_enabled", "powershell_logging",
    "script_block_logging", "amsi_enabled", "credential_guard",
]

def generate_sample(is_edr_present: bool, is_backup_present: bool) -> dict:
    sample = {}
    sample["process_count"] = random.randint(50, 300)
    sample["network_connections"] = random.randint(10, 200)
    sample["cpu_usage"] = round(random.uniform(5.0, 80.0), 2)
    sample["memory_usage"] = round(random.uniform(20.0, 90.0), 2)
    sample["disk_io"] = random.randint(100, 5000)
    sample["has_edr_process"] = 1 if is_edr_present else 0
    sample["has_backup_process"] = 1 if is_backup_present else 0
    sample["admin_privileges"] = random.choice([0, 1])
    sample["domain_joined"] = random.choice([0, 1])
    sample["firewall_enabled"] = random.choice([0, 1])
    sample["powershell_logging"] = random.choice([0, 1])
    sample["script_block_logging"] = random.choice([0, 1])
    sample["amsi_enabled"] = random.choice([0, 1])
    sample["credential_guard"] = random.choice([0, 1])

    if is_edr_present:
        sample["cpu_usage"] = min(sample["cpu_usage"] + random.uniform(2.0, 8.0), 100.0)
        sample["memory_usage"] = min(sample["memory_usage"] + random.uniform(3.0, 10.0), 100.0)

    label = 2 if is_edr_present else (1 if is_backup_present else 0)
    sample["label"] = label
    return sample

def main():
    output_dir = os.path.dirname(os.path.abspath(__file__))
    output_path = os.path.join(output_dir, "scout_dataset.csv")

    samples = []
    for _ in range(2000):
        samples.append(generate_sample(False, False))
    for _ in range(500):
        samples.append(generate_sample(True, False))
    for _ in range(300):
        samples.append(generate_sample(False, True))
    for _ in range(200):
        samples.append(generate_sample(True, True))

    random.shuffle(samples)

    with open(output_path, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=FEATURES + ["label"])
        writer.writeheader()
        writer.writerows(samples)

    print(f"Generated {len(samples)} samples -> {output_path}")
    print(f"  Normal: {sum(1 for s in samples if s['label'] == 0)}")
    print(f"  Backup: {sum(1 for s in samples if s['label'] == 1)}")
    print(f"  EDR:    {sum(1 for s in samples if s['label'] == 2)}")

if __name__ == "__main__":
    main()
