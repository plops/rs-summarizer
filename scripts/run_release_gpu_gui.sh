#!/usr/bin/env bash
set -euo pipefail

# Build and run the viz-tool (release) with GPU + GUI features
# Usage: ./run_release_gpu_gui.sh [path-to-db]
# If no DB path is provided, defaults to /home/kiel/stage/rs-summarizer/data/summaries.db

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

DB_PATH="${1:-/home/kiel/stage/rs-summarizer/data/summaries.db}"

echo "[run_release_gpu_gui] Repository root: $ROOT_DIR"
echo "[run_release_gpu_gui] Database: $DB_PATH"

echo "[run_release_gpu_gui] Building viz-tool (release) with features: gpu,gui"
cargo build --release --manifest-path viz-tool/Cargo.toml --features "gpu,gui"

BINARY="$ROOT_DIR/target/release/viz-tool"
if [ ! -x "$BINARY" ]; then
  echo "[run_release_gpu_gui] ERROR: binary not found at $BINARY"
  exit 2
fi

echo "[run_release_gpu_gui] Launching viz-tool GUI (this will block until the GUI exits)"
# Choose a sensible number of Rayon threads (use all logical cores by default)
NUM_THREADS="$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1)"
export RAYON_NUM_THREADS="$NUM_THREADS"
echo "[run_release_gpu_gui] Setting RAYON_NUM_THREADS=$RAYON_NUM_THREADS to accelerate CPU-bound phases (kNN / nn-descent)"
# Enable backtrace for debugging if the program panics
RUST_BACKTRACE=1 FAST_UMAP_NN_DESCENT_THRESHOLD=3000 "$BINARY" "$DB_PATH"

echo "[run_release_gpu_gui] viz-tool exited"
