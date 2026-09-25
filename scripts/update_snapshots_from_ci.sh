#!/usr/bin/env bash
# Replaces your existing snapshots with the ones CI produced for your branch.
#
# In the reality repo, this downloads the snapshots that the Buildkite `rust-tests-linux` job publishes
# to build.rerun.io for the commit (see `.buildkite/jobs/rerun/checks/rust_tests.ts`).
# Set `COMMIT` to use another commit than `HEAD`.
#
# Elsewhere, or when `RUN_ID` is set, this downloads the `test-results-linux` artifact of a GitHub Actions run:
# the latest one on your branch, unless `RUN_ID` says which.
# That requires the gh cli, installed and authenticated.

set -eu

# remove any existing .new.png that might have been left behind
find . -type d -path "*/tests/snapshots*" | while read -r dir; do
    find "$dir" -type f -name "*.new.png" | while read -r file; do
        rm "$file"
    done
done

if [ -n "${RUN_ID:-}" ] || [ ! -d "$(git rev-parse --show-toplevel)/.buildkite" ]; then
    if [ -z "${RUN_ID:-}" ]; then
        BRANCH="$(git rev-parse --abbrev-ref HEAD)"
        RUN_ID="$(gh api "repos/{owner}/{repo}/actions/artifacts?name=test-results-linux&per_page=100" \
            --jq "[.artifacts[] | select(.workflow_run.head_branch == \"$BRANCH\")][0].workflow_run.id // empty")"
        if [ -z "$RUN_ID" ]; then
            echo "No GitHub Actions run on branch $BRANCH has uploaded test results." >&2
            exit 1
        fi
    fi
    echo "Downloading test results from GitHub Actions run $RUN_ID"
    gh run download "$RUN_ID" --name "test-results-linux" --dir tmp_artefacts

    # move the snapshots to the correct location, overwriting the existing ones
    rsync -a tmp_artefacts/ .
    rm -r tmp_artefacts
else
    COMMIT="${COMMIT:-$(git rev-parse HEAD)}"
    SHORT_COMMIT="${COMMIT:0:7}"
    URL="https://build.rerun.io/buildkite/commit/$SHORT_COMMIT/snapshots/linux.tar.gz"
    ARCHIVE="$(mktemp)"
    trap 'rm -f "$ARCHIVE"' EXIT

    echo "Downloading snapshots for commit $SHORT_COMMIT from $URL"
    STATUS="$(curl --silent --show-error --location --output "$ARCHIVE" --write-out '%{http_code}' "$URL")"
    if [ "$STATUS" = "404" ]; then
        echo "No snapshots published for commit $SHORT_COMMIT." >&2
        echo "They are published when the Buildkite job 'rust-tests-linux' finishes running the tests for that commit." >&2
        echo "Wait for it to finish, then try again." >&2
        exit 1
    elif [ "$STATUS" != "200" ]; then
        echo "Downloading $URL failed with HTTP status $STATUS." >&2
        exit 1
    fi

    tar --exclude='*.diff.png' -xzf "$ARCHIVE"
fi

NUM_NEW="$(find . -type f -path "*/tests/snapshots*" -name "*.new.png" | wc -l | tr -d ' ')"
echo "Found $NUM_NEW new snapshot(s)"

./scripts/accept_snapshots.sh
