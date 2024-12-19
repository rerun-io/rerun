#!/usr/bin/env python3

"""Checks that the dev environment is set up correctly."""

from __future__ import annotations

import subprocess

PIXI_VERSION = "0.39.0"
CARGO_VERSION = "1.81.0"
RUST_VERSION = "1.81.0"


def check_version(cmd: str, expected: str, update: str, install: str) -> bool:
    try:
        output = subprocess.check_output([cmd, "--version"])

        version = output.strip().decode("utf-8").split(" ")[1]

        if version != expected:
            print(f"Expected {cmd} version {expected}, got {version}")
            print(f"Please run `{update}`")
            return False
        else:
            print(f"{cmd} version {version} is correct")
            return True

    except FileNotFoundError:
        print(f"{cmd} not found in PATH. Please install via {install}")
        return False


def main() -> int:
    success = True

    success &= check_version(
        "pixi",
        PIXI_VERSION,
        f"pixi self-update --version {PIXI_VERSION}",
        "https://pixi.sh/latest/",
    )

    success &= check_version(
        "cargo",
        CARGO_VERSION,
        f"rustup install {CARGO_VERSION}",
        "https://rustup.rs/",
    )

    success &= check_version(
        "rustc",
        RUST_VERSION,
        f"rustup install {RUST_VERSION}",
        "https://rustup.rs/",
    )

    if success:
        exit(0)
    else:
        exit(1)


if __name__ == "__main__":
    main()
