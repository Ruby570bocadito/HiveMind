"""
Train Scout classifier model and export ONNX + compact .bin formats.

The .bin (hive_base::ml custom format) is what the Rust worker actually
parses; ONNX remains only as an interop artifact. See ../export_bin.py.

Usage: python train_classifier.py
"""

import os
import subprocess
import sys

def main():
    try:
        import pandas as pd
        from sklearn.ensemble import RandomForestClassifier, GradientBoostingClassifier
        from sklearn.model_selection import train_test_split
        from sklearn.metrics import classification_report, accuracy_score
        import joblib
    except ImportError:
        print("Required packages not installed.")
        print("Run: pip install pandas scikit-learn joblib numpy")
        sys.exit(1)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    dataset_path = os.path.join(script_dir, "scout_dataset.csv")
    model_dir = os.path.join(script_dir, "..", "..", "agents", "worker", "models")
    os.makedirs(model_dir, exist_ok=True)

    if not os.path.exists(dataset_path):
        print(f"Dataset not found at {dataset_path}")
        print("Run generate_dataset.py first.")
        sys.exit(1)

    print("Loading dataset...")
    df = pd.read_csv(dataset_path)
    X = df.drop("label", axis=1)
    y = df["label"]

    X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)

    print(f"Training set: {len(X_train)}, Test set: {len(X_test)}")

    print("\nTraining Random Forest...")
    rf = RandomForestClassifier(n_estimators=100, max_depth=10, random_state=42)
    rf.fit(X_train, y_train)
    rf_pred = rf.predict(X_test)
    print(f"RF Accuracy: {accuracy_score(y_test, rf_pred):.4f}")
    print(classification_report(y_test, rf_pred, target_names=["normal", "backup", "edr"]))

    print("\nTraining Gradient Boosting...")
    gb = GradientBoostingClassifier(n_estimators=100, max_depth=5, random_state=42)
    gb.fit(X_train, y_train)
    gb_pred = gb.predict(X_test)
    print(f"GB Accuracy: {accuracy_score(y_test, gb_pred):.4f}")

    best_model = rf if accuracy_score(y_test, rf_pred) >= accuracy_score(y_test, gb_pred) else gb
    model_name = "RandomForest" if best_model is rf else "GradientBoosting"
    print(f"\nBest model: {model_name}")

    model_path = os.path.join(model_dir, "scout_classifier.joblib")
    joblib.dump(best_model, model_path)
    print(f"Model saved to {model_path}")

    # Always persist the RandomForest too: the .bin format consumed by
    # hive_base::ml (see training/export_bin.py) can only represent RF trees.
    rf_path = os.path.join(model_dir, "scout_classifier_rf.joblib")
    joblib.dump(rf, rf_path)

    try:
        from skl2onnx import convert_sklearn
        from skl2onnx.common.data_types import FloatTensorType
        initial_type = [("float_input", FloatTensorType([None, X.shape[1]]))]
        onnx_model = convert_sklearn(best_model, initial_types=initial_type, target_opset=12)
        onnx_path = os.path.join(model_dir, "scout_classifier.onnx")
        with open(onnx_path, "wb") as f:
            f.write(onnx_model.SerializeToString())
        print(f"ONNX model saved to {onnx_path}")
    except ImportError:
        print("skl2onnx not installed. Skipping ONNX export.")
        print("Run: pip install skl2onnx")

    # Export the compact .bin format that hive_base::ml::RandomForest::
    # from_binary parses (pure Rust, no ONNX Runtime). worker/build.rs
    # prefers this file over the ONNX automatically.
    export_script = os.path.join(script_dir, "..", "export_bin.py")
    bin_path = os.path.join(model_dir, "scout_classifier.bin")
    dataset_path_validate = dataset_path if os.path.exists(dataset_path) else None
    cmd = [sys.executable, os.path.abspath(export_script), os.path.abspath(rf_path),
           os.path.abspath(bin_path)]
    if dataset_path_validate:
        cmd += ["--validate", os.path.abspath(dataset_path_validate)]
    print("\nExporting compact .bin (hive_base::ml format)...")
    if subprocess.call(cmd) != 0:
        # Ronda 9: fallar en serio. El .bin es lo que el worker incrusta y
        # hive_base::ml no puede parsear ONNX, así que un export roto significa
        # scout degradado a heurísticas — en CI eso debe ser rojo, no un aviso
        # que nadie lee (y el .bin comprometido del checkout enmascararía el
        # fallo en un test de existencia).
        print("ERROR: .bin export/validation failed; refusing to ship a "
              "degraded scout. Fix the export (see export_bin.py).",
              file=sys.stderr)
        sys.exit(1)
    else:
        print(f"Compact .bin saved to {bin_path}")

    feature_names_path = os.path.join(model_dir, "feature_names.txt")
    with open(feature_names_path, "w") as f:
        for name in X.columns:
            f.write(f"{name}\n")
    print(f"Feature names saved to {feature_names_path}")

if __name__ == "__main__":
    main()
