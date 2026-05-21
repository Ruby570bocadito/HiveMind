"""
Train Shaper RL agent using DQN and export to ONNX.
Usage: python train_rl.py
"""

import os
import sys
import numpy as np

def main():
    try:
        import torch
        import torch.nn as nn
        import torch.optim as optim
        from env_simulator import NetworkEnvironment
    except ImportError:
        print("Required packages not installed.")
        print("Run: pip install torch numpy")
        sys.exit(1)

    model_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "agents", "zangano", "models")
    os.makedirs(model_dir, exist_ok=True)

    env = NetworkEnvironment(num_hosts=10, edr_coverage=0.3)
    state_size = env.state_size
    action_size = env.action_size

    class DQN(nn.Module):
        def __init__(self, state_size, action_size):
            super().__init__()
            self.network = nn.Sequential(
                nn.Linear(state_size, 128),
                nn.ReLU(),
                nn.Linear(128, 64),
                nn.ReLU(),
                nn.Linear(64, action_size),
            )

        def forward(self, x):
            return self.network(x)

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    policy_net = DQN(state_size, action_size).to(device)
    target_net = DQN(state_size, action_size).to(device)
    target_net.load_state_dict(policy_net.state_dict())
    optimizer = optim.Adam(policy_net.parameters(), lr=0.001)
    memory = []

    gamma = 0.99
    epsilon = 1.0
    epsilon_min = 0.01
    epsilon_decay = 0.995
    batch_size = 64
    num_episodes = 500

    print(f"Training DQN on device: {device}")
    print(f"State size: {state_size}, Action size: {action_size}")
    print(f"Episodes: {num_episodes}")

    for episode in range(num_episodes):
        state = env.reset()
        total_reward = 0.0
        done = False

        while not done:
            if random.random() < epsilon:
                action = random.randint(0, action_size - 1)
            else:
                with torch.no_grad():
                    state_t = torch.tensor(state, dtype=torch.float32).unsqueeze(0).to(device)
                    action = policy_net(state_t).argmax().item()

            next_state, reward, done, _ = env.step(action)
            memory.append((state, action, reward, next_state, done))
            if len(memory) > 10000:
                memory.pop(0)

            state = next_state
            total_reward += reward

            if len(memory) >= batch_size:
                batch = random.sample(memory, batch_size)
                states = torch.tensor([b[0] for b in batch], dtype=torch.float32).to(device)
                actions = torch.tensor([b[1] for b in batch], dtype=torch.long).unsqueeze(1).to(device)
                rewards = torch.tensor([b[2] for b in batch], dtype=torch.float32).to(device)
                next_states = torch.tensor([b[3] for b in batch], dtype=torch.float32).to(device)
                dones = torch.tensor([b[4] for b in batch], dtype=torch.float32).to(device)

                current_q = policy_net(states).gather(1, actions).squeeze()
                with torch.no_grad():
                    next_q = target_net(next_states).max(1)[0]
                expected_q = rewards + gamma * next_q * (1 - dones)

                loss = nn.MSELoss()(current_q, expected_q)
                optimizer.zero_grad()
                loss.backward()
                optimizer.step()

        epsilon = max(epsilon_min, epsilon * epsilon_decay)
        if episode % 10 == 0:
            target_net.load_state_dict(policy_net.state_dict())

        if episode % 50 == 0:
            print(f"Episode {episode}/{num_episodes} | Reward: {total_reward:.2f} | Epsilon: {epsilon:.3f}")

    torch.save(policy_net.state_dict(), os.path.join(model_dir, "shaper_policy.pt"))
    print(f"\nPolicy saved to {os.path.join(model_dir, 'shaper_policy.pt')}")

    dummy_input = torch.randn(1, state_size).to(device)
    onnx_path = os.path.join(model_dir, "shaper_policy.onnx")
    try:
        torch.onnx.export(
            policy_net.cpu(),
            dummy_input.cpu(),
            onnx_path,
            input_names=["state"],
            output_names=["q_values"],
            dynamic_axes={"state": {0: "batch"}, "q_values": {0: "batch"}},
            opset_version=12,
        )
        print(f"ONNX model saved to {onnx_path}")
    except Exception as e:
        print(f"ONNX export failed: {e}")

if __name__ == "__main__":
    import random
    main()
