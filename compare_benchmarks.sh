#!/bin/bash
# Script to compare benchmarks between baseline and current commits

set -e

BASELINE_COMMIT="cc3c97a20"
CURRENT_COMMIT="HEAD"

echo "=== Benchmark Comparison Script ==="
echo "Baseline: $BASELINE_COMMIT"
echo "Current:  $CURRENT_COMMIT"
echo ""

# Step 1: Checkout baseline and run benchmark
echo "Step 1: Running benchmark on baseline commit..."
git checkout "$BASELINE_COMMIT" 2>/dev/null || git checkout "$BASELINE_COMMIT"
cargo bench -p re_data_loader --bench parallel_ingestion_bench -- --output-format=bencher > /tmp/baseline.txt 2>&1
echo "✓ Baseline benchmark complete: /tmp/baseline.txt"
echo ""

# Step 2: Checkout current and run benchmark
echo "Step 2: Running benchmark on current commit..."
git checkout "$CURRENT_COMMIT" 2>/dev/null || git checkout "$CURRENT_COMMIT"
cargo bench -p re_data_loader --bench parallel_ingestion_bench -- --output-format=bencher > /tmp/current.txt 2>&1
echo "✓ Current benchmark complete: /tmp/current.txt"
echo ""

# Step 3: Compare results
echo "Step 3: Comparing results..."
if command -v cargo-benchcmp &> /dev/null; then
    cargo benchcmp /tmp/baseline.txt /tmp/current.txt
else
    echo "⚠ cargo-benchcmp not found. Install with: cargo install cargo-benchcmp"
    echo ""
    echo "Manual comparison:"
    echo "  Baseline: /tmp/baseline.txt"
    echo "  Current:  /tmp/current.txt"
fi

echo ""
echo "=== Done ==="

