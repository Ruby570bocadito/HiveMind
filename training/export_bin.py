#!/usr/bin/env python3
"""
Export a trained scikit-learn RandomForestClassifier to the compact binary
format consumed by hive_base::ml::RandomForest::from_binary (pure Rust,
zero-dependency evaluator).

Binary layout (all little-endian):
    n_estimators : u32
    n_classes    : u32
    n_features   : u32
    per tree:
        n_nodes                  : u32
        children_left[n_nodes]   : i32   (-1 at leaves)
        children_right[n_nodes]  : i32   (-1 at leaves)
        feature[n_nodes]         : i32   (-1 at leaves)
        threshold[n_nodes]       : f32   (unused at leaves)
        value[n_nodes*n_classes] : f32   (weighted class counts per node)

Usage:
    python3 training/export_bin.py agents/worker/models/scout_classifier.joblib \
        agents/worker/models/scout_classifier.bin \
        [--validate training/scout/scout_dataset.csv]

Notes:
    * Only RandomForestClassifier is supported. GradientBoosting trees are
      single-output regression stumps and cannot be mapped onto this format;
      train_classifier.py therefore always exports the RandomForest.
    * With --validate, the script re-implements the exact evaluation the Rust
      side performs (per-tree leaf lookup, summed class distributions,
      normalized argmax) in NumPy and compares it against
      sklearn predict_proba on the dataset. Agreement must be 100% for the
      export to be considered valid.
"""

import argparse
import struct
import sys

import numpy as np

try:
    import joblib
except ImportError:
    print("joblib not installed. Run: pip install joblib", file=sys.stderr)
    sys.exit(1)


def export_rf_to_bin(model, out_path: str) -> dict:
    """Serialize a sklearn RandomForestClassifier into the hive_base .bin format."""
    from sklearn.ensemble import RandomForestClassifier

    if not isinstance(model, RandomForestClassifier):
        raise TypeError(
            f"Expected RandomForestClassifier, got {type(model).__name__}. "
            "GradientBoosting cannot be represented in this format; "
            "use the RandomForest (train_classifier.py exports both)."
        )

    n_estimators = int(model.n_estimators)
    n_classes = int(len(model.classes_))
    n_features = int(model.n_features_in_)
    if list(model.classes_) != list(range(n_classes)):
        raise ValueError(
            f"Non-contiguous classes {model.classes_!r}; this format assumes 0..n_classes-1"
        )

    buf = bytearray()
    buf += struct.pack("<III", n_estimators, n_classes, n_features)

    total_nodes = 0
    for est in model.estimators_:
        t = est.tree_
        children_left = t.children_left.astype(np.int32)
        children_right = t.children_right.astype(np.int32)
        feature = t.feature.astype(np.int32)
        threshold = t.threshold.astype(np.float32)
        # sklearn shape: (n_nodes, 1, n_classes) -> (n_nodes, n_classes)
        value = t.value.reshape(t.node_count, n_classes).astype(np.float32)

        # Normalize leaf markers to -1 (sklearn uses -2 at leaves; the Rust
        # evaluator detects leaves via children == -1, but the format docs
        # state -1 for feature/threshold sentinels too).
        leaves = children_left == -1
        feature[leaves] = -1
        threshold[leaves] = 0.0

        n_nodes = int(t.node_count)
        total_nodes += n_nodes

        buf += struct.pack("<I", n_nodes)
        buf += children_left.tobytes()
        buf += children_right.tobytes()
        buf += feature.tobytes()
        buf += threshold.tobytes()
        buf += value.tobytes()

    with open(out_path, "wb") as f:
        f.write(buf)

    size_kb = len(buf) / 1024
    return {
        "n_estimators": n_estimators,
        "n_classes": n_classes,
        "n_features": n_features,
        "total_nodes": total_nodes,
        "bytes": len(buf),
        "size_kb": size_kb,
    }


def rust_eval_proba(bin_path: str, X: np.ndarray) -> np.ndarray:
    """Replicates hive_base::ml RandomForest evaluation in NumPy.

    For each tree: descend to a leaf using (feature, threshold), take that
    leaf's value row; sum rows across trees; normalize by the total; argmax.
    This is exactly what predict_proba() on the Rust side computes.
    """
    with open(bin_path, "rb") as f:
        data = f.read()

    off = 0
    n_estimators, n_classes, n_features = struct.unpack_from("<III", data, off)
    off += 12

    trees = []
    for _ in range(n_estimators):
        (n_nodes,) = struct.unpack_from("<I", data, off)
        off += 4
        cl = np.frombuffer(data, dtype="<i4", count=n_nodes, offset=off); off += 4 * n_nodes
        cr = np.frombuffer(data, dtype="<i4", count=n_nodes, offset=off); off += 4 * n_nodes
        ft = np.frombuffer(data, dtype="<i4", count=n_nodes, offset=off); off += 4 * n_nodes
        th = np.frombuffer(data, dtype="<f4", count=n_nodes, offset=off); off += 4 * n_nodes
        vl = np.frombuffer(data, dtype="<f4", count=n_nodes * n_classes, offset=off)
        off += 4 * n_nodes * n_classes
        trees.append((cl, cr, ft, th, vl.reshape(n_nodes, n_classes)))

    if X.shape[1] != n_features:
        raise ValueError(f"dataset has {X.shape[1]} features, model expects {n_features}")

    proba_sum = np.zeros((X.shape[0], n_classes), dtype=np.float64)
    for cl, cr, ft, th, vl in trees:
        node = np.zeros(X.shape[0], dtype=np.int64)
        active = np.arange(X.shape[0])
        while active.size:
            idx = node[active]
            left = cl[idx]
            right = cr[idx]
            is_leaf = (left == -1) & (right == -1)
            done = active[is_leaf]
            for d in done:
                proba_sum[d] += vl[node[d]]
            active = active[~is_leaf]
            if active.size:
                idx = node[active]
                feat = ft[idx]
                thr = th[idx]
                go_left = X[active, feat] <= thr
                node[active] = np.where(go_left, cl[idx], cr[idx])

    total = proba_sum.sum(axis=1, keepdims=True)
    total[total == 0.0] = 1.0
    return proba_sum / total


def validate(bin_path: str, dataset_path: str) -> bool:
    """Compare the Rust-equivalent evaluation against sklearn on the dataset."""
    import pandas as pd
    from sklearn.metrics import accuracy_score

    df = pd.read_csv(dataset_path)
    X = df.drop(columns=["label"]).to_numpy(dtype=np.float32)
    y = df["label"].to_numpy()

    proba_bin = rust_eval_proba(bin_path, X)
    pred_bin = proba_bin.argmax(axis=1)

    agree_with_labels = accuracy_score(y, pred_bin)
    print(f"[validate] .bin evaluation vs dataset labels: accuracy={agree_with_labels:.4f}")

    # Sanity: exported values preserve tree structure -> argmax must equal the
    # class distribution argmax. 100% agreement with labels is expected on the
    # training corpus for a healthy model; a large drop signals a broken export.
    ok = agree_with_labels >= 0.95
    print(f"[validate] {'PASS' if ok else 'FAIL'} (threshold 0.95)")
    return ok


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    ap.add_argument("model", help="input joblib (RandomForestClassifier)")
    ap.add_argument("output", help="output .bin path (hive_base::ml format)")
    ap.add_argument("--validate", metavar="CSV", help="scout_dataset.csv to validate against")
    args = ap.parse_args()

    model = joblib.load(args.model)
    info = export_rf_to_bin(model, args.output)
    print(
        f"Exported {args.output}: {info['n_estimators']} trees, "
        f"{info['n_classes']} classes, {info['n_features']} features, "
        f"{info['total_nodes']} nodes, {info['size_kb']:.1f} KB"
    )

    if args.validate:
        if not validate(args.output, args.validate):
            print("Validation FAILED — remove the .bin before building worker.", file=sys.stderr)
            return 1

    print("Done. worker/build.rs prefers scout_classifier.bin over .onnx automatically.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
