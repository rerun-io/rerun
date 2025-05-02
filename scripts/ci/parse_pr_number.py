#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys


def parse_pr_number(commit_message: str) -> int:
    first_line = commit_message.splitlines()[0]
    start_idx = first_line.rfind("(#")
    if start_idx == -1:
        raise Exception("failed to parse PR number: no PR number in commit message, expected to find '(#1234)'")
    start_idx += 2  # trim '(#'

    end_idx = first_line.find(")", start_idx)
    if end_idx == -1:
        raise Exception("failed to parse PR number: unclosed parenthesis, expected to find '(#1234)'")
    # end idx is exclusive, no need to trim

    digits = first_line[start_idx:end_idx]
    return int(digits)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("commit_message", type=str)

    args, unknown = parser.parse_known_args()
    for arg in unknown:
        print(f"Unknown argument: {arg}")

    pr_number = parse_pr_number(args.commit_message)
    sys.stdout.write(str(pr_number))
    sys.stdout.flush()


if __name__ == "__main__":
    main()
