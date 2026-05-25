#!/usr/bin/env python3
"""
Hive ML Pipeline: train EDR detection model and export to ONNX.
Run: python3 training/train_model.py [--epochs 50] [--output ../hive_base/models/edr_model.onnx]
"""
import argparse
import json
import os
import sys
import pickle

import numpy as np
from sklearn.ensemble import RandomForestClassifier
from sklearn.model_selection import train_test_split

try:
    from skl2onnx import convert_sklearn
    from skl2onnx.common.data_types import FloatTensorType
    HAS_ONNX = True
except ImportError:
    HAS_ONNX = False

SYNTHETIC_DATA = {
    "features": [
        # [proc_count, thread_count, handle_count, mem_mb, cpu_pct, disk_io, net_conns, running_time_s]
        # Non-EDR (benign)
        [45, 120, 800, 256, 12, 50, 5, 3600],
        [32, 90, 600, 180, 8, 30, 3, 7200],
        [55, 150, 950, 320, 15, 70, 8, 5400],
        [28, 80, 500, 140, 6, 25, 2, 9000],
        [60, 170, 1100, 400, 18, 90, 10, 1800],
        [38, 100, 700, 220, 10, 45, 4, 6000],
        [50, 130, 850, 290, 13, 55, 6, 4200],
        [42, 110, 750, 240, 11, 40, 4, 8000],
        [35, 95, 650, 200, 9, 35, 3, 10000],
        [48, 125, 820, 270, 12, 48, 5, 4800],
        # EDR present
        [120, 350, 2500, 800, 35, 200, 25, 100],
        [150, 420, 3000, 950, 42, 250, 35, 80],
        [90, 280, 2000, 600, 28, 150, 18, 200],
        [200, 500, 4000, 1200, 55, 350, 50, 60],
        [80, 250, 1800, 500, 22, 120, 15, 300],
        [170, 450, 3500, 1100, 48, 300, 40, 50],
        [110, 320, 2200, 700, 32, 180, 22, 150],
        [130, 380, 2700, 850, 38, 220, 28, 120],
        [95, 300, 2100, 650, 30, 160, 20, 180],
        [160, 430, 3200, 1000, 45, 280, 35, 70],
    ],
    "labels": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
}

def generate_diverse_data(n_samples=1000):
    np.random.seed(42)
    n_benign = n_samples // 2
    n_edr = n_samples - n_benign

    benign = np.column_stack([
        np.random.normal(40, 10, n_benign).clip(10, 80),      # procs
        np.random.normal(110, 25, n_benign).clip(50, 200),    # threads
        np.random.normal(750, 150, n_benign).clip(300, 1200), # handles
        np.random.normal(250, 60, n_benign).clip(100, 500),   # mem
        np.random.normal(11, 3, n_benign).clip(3, 25),        # cpu
        np.random.normal(45, 15, n_benign).clip(10, 100),     # disk
        np.random.normal(5, 2, n_benign).clip(0, 15),         # net
        np.random.exponential(5000, n_benign).clip(60, 20000),# runtime
    ])

    edr = np.column_stack([
        np.random.normal(130, 35, n_edr).clip(60, 250),
        np.random.normal(360, 80, n_edr).clip(180, 600),
        np.random.normal(2600, 600, n_edr).clip(1200, 5000),
        np.random.normal(800, 200, n_edr).clip(400, 1500),
        np.random.normal(38, 10, n_edr).clip(18, 65),
        np.random.normal(220, 70, n_edr).clip(80, 400),
        np.random.normal(28, 10, n_edr).clip(10, 60),
        np.random.exponential(150, n_edr).clip(30, 600),
    ])

    X = np.vstack([benign, edr])
    y = np.hstack([np.zeros(n_benign), np.ones(n_edr)])

    shuffle = np.random.permutation(n_samples)
    return X[shuffle], y[shuffle]

FEATURE_NAMES = [
    "process_count", "thread_count", "handle_count",
    "memory_mb", "cpu_percent", "disk_io_kbps",
    "network_connections", "running_time_seconds",
]

def main():
    parser = argparse.ArgumentParser(description="Hive ML Pipeline")
    parser.add_argument("--epochs", type=int, default=50, help="Number of trees")
    parser.add_argument("--output", default="hive_base/models/edr_model.onnx",
                        help="Output ONNX model path")
    parser.add_argument("--samples", type=int, default=500,
                        help="Number of synthetic samples to generate")
    args = parser.parse_args()

    print(f"Hive ML Pipeline: training EDR classifier")
    print(f"  Trees: {args.epochs}, Samples: {args.samples}")

    X, y = generate_diverse_data(args.samples)
    X_train, X_test, y_train, y_test = train_test_split(
        X, y, test_size=0.2, random_state=42
    )

    model = RandomForestClassifier(
        n_estimators=args.epochs,
        max_depth=12,
        min_samples_split=4,
        random_state=42,
        n_jobs=-1,
    )
    model.fit(X_train, y_train)

    train_score = model.score(X_train, y_train)
    test_score = model.score(X_test, y_test)
    print(f"  Train accuracy: {train_score:.3f}")
    print(f"  Test accuracy:  {test_score:.3f}")

    feature_importance = list(zip(FEATURE_NAMES, model.feature_importances_))
    feature_importance.sort(key=lambda x: -x[1])
    print("  Top features:")
    for name, imp in feature_importance[:4]:
        print(f"    {name}: {imp:.3f}")

    os.makedirs(os.path.dirname(args.output), exist_ok=True)

    # Save as pickle as well
    pkl_path = args.output.replace(".onnx", ".pkl")
    with open(pkl_path, "wb") as f:
        pickle.dump(model, f)
    print(f"  Pickle: {pkl_path}")

    # Export to ONNX
    if HAS_ONNX:
        initial_types = [("input", FloatTensorType([None, 8]))]
        onnx_model = convert_sklearn(model, initial_types=initial_types)
        with open(args.output, "wb") as f:
            f.write(onnx_model.SerializeToString())
        print(f"  ONNX:   {args.output}")
    else:
        print("  skl2onnx not installed; saving pickle only")
        print(f"  Install: pip install skl2onnx onnxruntime")

    # Save metadata
    meta = {
        "model": os.path.basename(args.output),
        "type": "RandomForest",
        "features": FEATURE_NAMES,
        "input_dim": 8,
        "n_trees": args.epochs,
        "train_accuracy": float(train_score),
        "test_accuracy": float(test_score),
        "feature_importance": dict(feature_importance),
    }
    meta_path = args.output.replace(".onnx", ".json").replace(".pkl", ".json")
    with open(meta_path, "w") as f:
        json.dump(meta, f, indent=2)
    print(f"  Metadata: {meta_path}")
    print("Done.")


if __name__ == "__main__":
    main()
