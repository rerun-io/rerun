#!/usr/bin/env bash
# Rust test coverage via cargo-llvm-cov + nextest.
#
# Run through pixi so the coverage tools are on PATH (auto-selects the `coverage` environment):
#   pixi run rs-coverage                 # whole workspace
#   pixi run rs-coverage re_dataframe    # scoped to one crate
#
# Runs the test suite once under source-based coverage instrumentation, then emits every
# report format from that single run so the suite isn't executed more than once:
#   * lcov.info                        — line coverage consumed by the "Coverage Gutters"
#                                        VS Code extension (recommended in
#                                        `.vscode/extensions.json`; it finds `lcov.info`
#                                        out of the box — open a file and run
#                                        "Coverage Gutters: Watch" to see inline gutters).
#   * target/llvm-cov/html/index.html  — browsable HTML report (path printed at the end).
#   * a per-file coverage summary table printed to the terminal.
#
# `cargo-llvm-cov` needs the `llvm-tools-preview` rustup component matching the pinned
# toolchain, which we add on the fly (idempotent, same idea as `rerun-build-web` adding
# the wasm target).
#
# `--ignore-run-fail` means a failing test still produces a coverage report (test failures
# are printed but don't abort the run) — useful because a whole-workspace run includes GPU
# image-snapshot tests (e.g. re_integration_test) that are environment-sensitive locally.
# For fast, clean local coverage, scope to a crate: `pixi run rs-coverage re_dataframe`.

set -eu

sel=("${1:---workspace}")
case "${sel[0]}" in
    --*) ;;
    *) sel=(-p "${sel[0]}") ;;
esac

# Install the component `cargo-llvm-cov` needs to instrument builds (see comment above).
rustup component add llvm-tools-preview

# Run the tests once under coverage instrumentation, without generating a report yet.
cargo llvm-cov nextest --ignore-run-fail --all-features --no-report "${sel[@]}"
# Emit the report formats below from that single run's coverage data.
cargo llvm-cov report --lcov --output-path lcov.info   # for Coverage Gutters
cargo llvm-cov report --html                           # browsable HTML report
cargo llvm-cov report                                   # per-file summary table to stdout
echo "HTML report: $PWD/target/llvm-cov/html/index.html"
