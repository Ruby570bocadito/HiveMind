"""
Train Scout classifier model and export to ONNX format.
Usage: python train_classifier.py
"""

import os
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
        print("Run: pip install pandas scikit-learn joblib onnx onnxruntime skl2onnx")
        sys.exit(1)

    script_dir = os.path.dirname(os.path.abspath(__file__))
    dataset_path = os.path.join(script_dir, "scout_dataset.csv")
    model_dir = os.path.join(script_dir, "..", "..", "agents", "obrera", "models")
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

    feature_names_path = os.path.join(model_dir, "feature_names.txt")
    with open(feature_names_path, "w") as f:
        for name in X.columns:
            f.write(f"{name}\n")
    print(f"Feature names saved to {feature_names_path}")

if __name__ == "__main__":
    main()
