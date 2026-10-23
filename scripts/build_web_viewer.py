#!/usr/bin/env python3
"""Build `re_viewer` as Wasm, and generate the `.js` bindings for it with `wasm-bindgen`."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
import tomllib
from pathlib import Path

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
CRATE_NAME = "re_viewer"
DEFAULT_OUT_DIR = WORKSPACE_ROOT / "crates" / "top" / "re_web_viewer_server" / "web_viewer"


def run(cmd: list[str], env: dict[str, str] | None = None) -> None:
    print(f"{WORKSPACE_ROOT}> {' '.join(cmd)}", flush=True)
    subprocess.run(cmd, cwd=WORKSPACE_ROOT, env=env, check=True)


def locked_wasm_bindgen_version() -> str:
    lock = tomllib.loads((WORKSPACE_ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    versions: set[str] = {package["version"] for package in lock["package"] if package["name"] == "wasm-bindgen"}
    if len(versions) != 1:
        sys.exit(f"Expected exactly one version of `wasm-bindgen` in Cargo.lock, found {sorted(versions)}")
    return versions.pop()


def check_wasm_bindgen_cli() -> None:
    expected = locked_wasm_bindgen_version()
    output = subprocess.run(["wasm-bindgen", "--version"], capture_output=True, text=True, check=True).stdout
    actual = output.split()[-1]
    if actual != expected:
        sys.exit(
            f"`wasm-bindgen` CLI is version {actual}, but Cargo.lock uses `wasm-bindgen` {expected}. "
            "Update the `wasm-bindgen-cli` version in `pixi.toml` to match."
        )


def wasm_target_dir() -> Path:
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--no-deps"],
        cwd=WORKSPACE_ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return Path(json.loads(metadata)["target_directory"]) / "wasm"


def build_wasm(target_dir: Path, profile: str, features: str, timings: bool) -> Path:
    cmd = [
        "cargo",
        "rustc",
        f"--package={CRATE_NAME}",
        "--lib",
        "--crate-type=cdylib",
        "--target=wasm32-unknown-unknown",
        f"--target-dir={target_dir}",
        "--no-default-features",
        "--config=.cargo/config.toml",
    ]
    if features:
        cmd.append(f"--features={features}")
    if profile == "web-release":
        cmd.append("--profile=web-release")
    if timings:
        cmd.append("--timings")

    env = {k: v for k, v in os.environ.items() if k not in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")}
    run(cmd, env=env)

    return target_dir / "wasm32-unknown-unknown" / profile / f"{CRATE_NAME}.wasm"


def generate_bindings(wasm_path: Path, out_dir: Path, target: str) -> None:
    cmd = [
        "wasm-bindgen",
        str(wasm_path),
        f"--out-dir={out_dir}",
        f"--out-name={CRATE_NAME}",
        "--omit-default-module-path",
    ]
    if target == "browser":
        cmd += ["--target=no-modules", "--no-typescript"]
    elif target == "module":
        cmd += ["--target=bundler"]
    elif target == "no-modules-base":
        cmd += ["--target=no-modules"]

    try:
        run(cmd)
    except subprocess.CalledProcessError:
        sys.exit(
            "Failed to run wasm-bindgen. If it complains about imports from `env`, some dependency is likely "
            f"calling `std::time::Instant::now()` or similar. Diagnose with: wasm2wat {wasm_path} | rg '\"env\"'"
        )


def optimize_wasm(wasm_path: Path, debug_symbols: bool) -> None:
    cmd = [
        "wasm-opt",
        str(wasm_path),
        "-O2",
        "--output",
        str(wasm_path),
        "--enable-reference-types",
        "--enable-simd",
        "--enable-bulk-memory",
        "--enable-nontrapping-float-to-int",
        "--enable-multivalue",
        "--vacuum",
        "-g" if debug_symbols else "--strip-debug",
    ]
    run(cmd)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--release", action="store_true", help="Build with the `web-release` profile.")
    mode.add_argument("--debug", action="store_true", help="Build with the `dev` profile.")
    parser.add_argument(
        "-g",
        "--debug-symbols",
        action="store_true",
        help="Keep debug symbols in release builds, for readable callstacks and in-browser profiling.",
    )
    parser.add_argument(
        "-t",
        "--target",
        choices=["browser", "module", "no-modules-base"],
        default="browser",
        help="`no-modules-base` is meant for post-processing in `rerun_js`.",
    )
    parser.add_argument(
        "-o",
        "--out",
        type=Path,
        default=DEFAULT_OUT_DIR,
        help="Output directory, relative to the workspace root.",
    )
    parser.add_argument(
        "-F",
        "--features",
        default="analytics,default_web",
        help="Comma-separated features of `re_viewer`, used instead of its default features.",
    )
    parser.add_argument("--timings", action="store_true", help="Generate a cargo timings report.")
    args = parser.parse_args()

    check_wasm_bindgen_cli()

    out_dir = WORKSPACE_ROOT / args.out
    if not out_dir.is_dir():
        sys.exit(f"Output directory {out_dir} does not exist")
    for name in (f"{CRATE_NAME}_bg.wasm", f"{CRATE_NAME}.js"):
        (out_dir / name).unlink(missing_ok=True)

    profile = "web-release" if args.release else "debug"

    start = time.monotonic()
    wasm_path = build_wasm(wasm_target_dir(), profile, args.features, args.timings)
    print(f"Built .wasm in {time.monotonic() - start:.1f}s", flush=True)

    start = time.monotonic()
    generate_bindings(wasm_path, out_dir, args.target)
    print(f"Generated JS bindings in {time.monotonic() - start:.1f}s", flush=True)

    if args.release:
        start = time.monotonic()
        optimize_wasm(out_dir / f"{CRATE_NAME}_bg.wasm", args.debug_symbols)
        print(f"Optimized wasm in {time.monotonic() - start:.1f}s", flush=True)

    print(f"Finished {out_dir / f'{CRATE_NAME}_bg.wasm'}")


if __name__ == "__main__":
    main()
