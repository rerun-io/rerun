#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import os
import sys


def compute_file_hash(file_path: str) -> str:
    sha256 = hashlib.sha256()
    relative_path = os.path.basename(file_path)
    sha256.update(relative_path.encode())  # Include file name in the hash

    with open(file_path, "rb") as f:
        while chunk := f.read(8192):
            sha256.update(chunk)

    return sha256.hexdigest()


def compute_directory_hash(directory: str) -> str:
    sha256 = hashlib.sha256()

    for root, _, files in sorted(os.walk(directory)):
        for filename in sorted(files):
            filepath = os.path.join(root, filename)
            relative_path = os.path.relpath(filepath, directory)

            sha256.update(relative_path.encode())  # Include file path in the hash

            with open(filepath, "rb") as f:
                while chunk := f.read(8192):
                    sha256.update(chunk)

    return sha256.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Compute the SHA-256 hash of a file or directory and optionally verify it."
    )
    parser.add_argument("path", help="Path to the file or directory to hash.")
    parser.add_argument(
        "expected_hash", nargs="?", default=None, help="Optional expected SHA-256 hash to verify against."
    )
    args = parser.parse_args()

    if os.path.isfile(args.path):
        actual_hash = compute_file_hash(args.path)
    elif os.path.isdir(args.path):
        actual_hash = compute_directory_hash(args.path)
    else:
        print(f"❌ Error: {args.path} is not a valid file or directory.")
        sys.exit(2)

    if args.expected_hash:
        print(f"Computed hash: {actual_hash}")
        print(f"Expected hash: {args.expected_hash}")

        if actual_hash != args.expected_hash:
            print("❌ Hash mismatch!")
            sys.exit(1)

        print("✅ Hash matches!")
    else:
        print(actual_hash)

    sys.exit(0)


if __name__ == "__main__":
    main()
