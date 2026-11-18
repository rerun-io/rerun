#!/usr/bin/env bash
# Runs `buf breaking` against the merge-base of HEAD and `origin/${BASE_BRANCH:-main}`, which must be fetched.
# Only breaking changes made since the branch forked from the base branch are reported.
# Without that merge-base in the local history, as in a shallow clone, compares against the tip of the base branch.
#
# Arguments are passed on to `buf breaking`.
set -euo pipefail

base="origin/${BASE_BRANCH:-main}"
if ! against="$(git merge-base HEAD "$base")"; then
  echo "⚠️ Could not find the merge-base of HEAD and $base, comparing against the tip of $base instead." >&2
  against="$base"
fi

echo "ℹ️ Checking for breaking Protobuf changes against $against."
buf breaking --error-format=json --against ".git#ref=$against" "$@"
