"""
Simulated corporate network environment for Shaper RL training.
"""

import numpy as np
import random

class NetworkEnvironment:
    def __init__(self, num_hosts=20, edr_coverage=0.3):
        self.num_hosts = num_hosts
        self.edr_coverage = edr_coverage
        self.reset()

    def reset(self):
        self.hosts = []
        for i in range(self.num_hosts):
            self.hosts.append({
                "id": i,
                "compromised": i == 0,
                "has_edr": random.random() < self.edr_coverage,
                "has_backup": random.random() < 0.2,
                "value": random.uniform(0.1, 1.0),
                "segment": random.randint(0, 3),
                "latency": random.uniform(1, 100),
            })
        self.current_host = 0
        self.steps = 0
        self.max_steps = 100
        self.detected = False
        return self._get_state()

    def _get_state(self):
        state = []
        for h in self.hosts:
            state.extend([
                float(h["compromised"]),
                float(h["has_edr"]),
                float(h["has_backup"]),
                h["value"],
                float(h["segment"]),
                h["latency"] / 100.0,
            ])
        state.append(float(self.current_host) / self.num_hosts)
        state.append(float(self.steps) / self.max_steps)
        state.append(float(self.detected))
        return np.array(state, dtype=np.float32)

    def step(self, action):
        self.steps += 1
        reward = 0.0
        done = False

        target = action % self.num_hosts

        if target == self.current_host:
            reward = -0.1
        elif self.hosts[target]["has_edr"]:
            if random.random() < 0.4:
                self.detected = True
                reward = -10.0
                done = True
            else:
                self.hosts[target]["compromised"] = True
                reward = self.hosts[target]["value"] * 2.0
        else:
            self.hosts[target]["compromised"] = True
            reward = self.hosts[target]["value"] * 3.0

        self.current_host = target
        compromised_count = sum(1 for h in self.hosts if h["compromised"])
        if compromised_count == self.num_hosts:
            reward += 20.0
            done = True

        if self.steps >= self.max_steps:
            done = True

        return self._get_state(), reward, done, {}

    @property
    def state_size(self):
        return len(self._get_state())

    @property
    def action_size(self):
        return self.num_hosts
