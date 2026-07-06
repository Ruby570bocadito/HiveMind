#!/usr/bin/env python3
"""
Multi-Agent RL training with PPO via stable-baselines3.
Trains the Shaper's decision policy on a simulated corporate network.
Exports ONNX for embedding in the Rust binary.

Requires: pip install stable-baselines3 gymnasium numpy onnx onnxruntime

Usage:
    python3 train_marl.py [--episodes 50000] [--export shaper_policy.onnx]
"""

import numpy as np
import gymnasium as gym
from gymnasium import spaces
from stable_baselines3 import PPO
from stable_baselines3.common.vec_env import DummyVecEnv
from stable_baselines3.common.callbacks import EvalCallback
import argparse
import os

# ── Corporate Network Simulator ───────────────────────────────────────────────

class CorporateNetworkEnv(gym.Env):
    """
    10-host corporate network with EDR, segments, and value scores.
    Agent actions: propagate to host 0-8, install persistence on host 9.
    Reward: +value_score for compromise, -penalty for detection.
    """
    metadata = {'render_modes': ['human']}

    def __init__(self, num_hosts=10):
        super().__init__()
        self.num_hosts = num_hosts
        self.max_steps = 100

        # Action: 0-8 propagate to host_i, 9 persist on current
        self.action_space = spaces.Discrete(10)

        # State: 10 hosts * 6 features + 2 global = 62
        self.observation_space = spaces.Box(
            low=0.0, high=1.0, shape=(62,), dtype=np.float32
        )
        self.reset()

    def reset(self, seed=None, options=None):
        super().reset(seed=seed)
        np.random.seed(seed if seed is not None else None)

        self.hosts = []
        roles = ['workstation'] * 6 + ['server'] * 2 + ['dc'] * 1 + ['backup'] * 1
        np.random.shuffle(roles)

        for i in range(self.num_hosts):
            edr = np.random.choice([0.0, 0.3, 0.7, 1.0], p=[0.35, 0.3, 0.25, 0.1])
            self.hosts.append({
                'has_agent': 1.0 if i == 0 else 0.0,
                'edr_level': edr,
                'backup_role': 1.0 if roles[i] == 'backup' else 0.0,
                'segment': float(i % 3),
                'value_score': np.random.uniform(0.1, 1.0),
                'compromised': 1.0 if i == 0 else 0.0,
                'reachable_from': [],
            })

        # Build topology: same segment = reachable, DC is hub
        for i in range(self.num_hosts):
            for j in range(self.num_hosts):
                if i != j:
                    same_seg = self.hosts[i]['segment'] == self.hosts[j]['segment']
                    is_dc = self.hosts[j]['segment'] == 2.0
                    if same_seg or is_dc:
                        self.hosts[i]['reachable_from'].append(j)

        self.detection_level = 0.0
        self.alerts_active = 0.0
        self.step_count = 0
        return self._get_state(), {}

    def _get_state(self):
        state = np.zeros(62, dtype=np.float32)
        for i, host in enumerate(self.hosts):
            base = i * 6
            state[base] = host['has_agent']
            state[base + 1] = host['edr_level']
            state[base + 2] = host['backup_role']
            state[base + 3] = host['segment'] / 3.0
            state[base + 4] = host['value_score']
            state[base + 5] = host['compromised']
        state[60] = self.detection_level
        state[61] = self.alerts_active
        return state

    def step(self, action):
        self.step_count += 1
        reward = 0.0

        if action <= 8:
            target = action
            agents_present = [i for i, h in enumerate(self.hosts) if h['has_agent'] > 0.5]
            can_reach = any(target in self.hosts[i].get('reachable_from', []) for i in agents_present)

            if can_reach and self.hosts[target]['compromised'] < 0.5:
                edr = self.hosts[target]['edr_level']
                success_prob = max(0.05, 1.0 - edr * 0.85)

                if np.random.random() < success_prob:
                    self.hosts[target]['has_agent'] = 1.0
                    self.hosts[target]['compromised'] = 1.0
                    bonus = self.hosts[target]['value_score'] * 10.0
                    if self.hosts[target]['backup_role'] > 0.5: bonus += 5.0
                    if self.hosts[target]['segment'] == 2.0: bonus += 8.0
                    reward += bonus
                else:
                    self.detection_level = min(1.0, self.detection_level + 0.15)
                    self.alerts_active = 1.0
                    reward -= 2.0

        elif action == 9:
            for host in self.hosts:
                if host['has_agent'] > 0.5:
                    if np.random.random() < (1.0 - host['edr_level'] * 0.5):
                        reward += 2.0
                    else:
                        self.detection_level = min(1.0, self.detection_level + 0.1)
                        reward -= 1.0

        reward -= 0.05  # time penalty
        self.detection_level = max(0.0, self.detection_level - 0.02)
        if self.detection_level < 0.1:
            self.alerts_active = 0.0

        compromised = sum(1 for h in self.hosts if h['compromised'] > 0.5)
        terminated = compromised >= self.num_hosts * 0.8 or self.detection_level >= 0.95
        truncated = self.step_count >= self.max_steps

        return self._get_state(), reward, terminated, truncated, {
            'compromised': compromised,
            'detection': self.detection_level,
        }

# ── Training ──────────────────────────────────────────────────────────────────

def train_ppo(episodes=50000, export_path='shaper_policy_ppo'):
    env = CorporateNetworkEnv(num_hosts=10)
    env = DummyVecEnv([lambda: env])

    model = PPO(
        'MlpPolicy',
        env,
        learning_rate=3e-4,
        n_steps=2048,
        batch_size=64,
        n_epochs=10,
        gamma=0.99,
        gae_lambda=0.95,
        clip_range=0.2,
        ent_coef=0.01,
        verbose=1,
        tensorboard_log='./ppo_tensorboard/',
    )

    eval_env = DummyVecEnv([lambda: CorporateNetworkEnv(num_hosts=10)])
    eval_callback = EvalCallback(
        eval_env,
        best_model_save_path='./ppo_best/',
        log_path='./ppo_logs/',
        eval_freq=5000,
        deterministic=True,
    )

    print(f"Training PPO for {episodes} timesteps...")
    model.learn(total_timesteps=episodes, callback=eval_callback)

    model.save(export_path)
    print(f"Model saved: {export_path}.zip")

    # Export ONNX
    try:
        import onnx
        import onnxruntime as ort
        onnx_path = export_path + '.onnx'
        model.policy.to(device='cpu')
        dummy_input = np.random.randn(1, 62).astype(np.float32)

        import torch
        torch.onnx.export(
            model.policy,
            torch.from_numpy(dummy_input),
            onnx_path,
            input_names=['input'],
            output_names=['output'],
            dynamic_axes={'input': {0: 'batch'}, 'output': {0: 'batch'}},
        )
        print(f"ONNX exported: {onnx_path}")
    except Exception as e:
        print(f"ONNX export skipped: {e}")
        print("Run: pip install onnx onnxruntime torch")

    return model

# ── Evaluation ────────────────────────────────────────────────────────────────

def evaluate(model, episodes=10):
    env = CorporateNetworkEnv(num_hosts=10)
    rewards = []
    compromises = []

    for ep in range(episodes):
        obs, _ = env.reset()
        total_r = 0
        done = False
        truncated = False
        while not done and not truncated:
            action, _ = model.predict(obs, deterministic=True)
            obs, r, done, truncated, info = env.step(action)
            total_r += r
        rewards.append(total_r)
        compromises.append(info.get('compromised', 0))

    print(f"\nEvaluation ({episodes} episodes):")
    print(f"  Mean reward: {np.mean(rewards):.1f} ± {np.std(rewards):.1f}")
    print(f"  Mean compromised hosts: {np.mean(compromises):.1f}/{env.num_hosts}")

# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description='PPO MARL Training for Colmena-Shaper')
    parser.add_argument('--episodes', type=int, default=50000, help='Training timesteps')
    parser.add_argument('--export', type=str, default='shaper_policy_ppo', help='Model save path')
    parser.add_argument('--load', type=str, help='Load existing model for fine-tuning')
    parser.add_argument('--eval-only', action='store_true', help='Evaluate only, no training')
    args = parser.parse_args()

    if args.eval_only and args.load:
        model = PPO.load(args.load)
        evaluate(model)
        return

    model = train_ppo(args.episodes, args.export)
    evaluate(model)

if __name__ == '__main__':
    main()
