#!/usr/bin/env bash
# Delete snapshot baselines that CI has no reason to upload.
#
# `egui_kittest` writes `{name}.new.png` (and often `{name}.diff.png`) next to the
# checked-in `{name}.png` when a snapshot test fails or when the snapshot is new.
# It writes `{name}.old.png` instead when run in update mode.
#
# Every other `{name}.png` passed its test, so both kitdiff and
# `update_snapshots_from_ci.sh` ignore it. Uploading it wastes time and storage.
#
# Run this from the repository root, after the tests and before the artifact upload.

set -eu

kept=0
deleted=0

find_snapshot_pngs() {
    find . -type d -path "*/tests/snapshots" | while read -r dir; do
        find "$dir" -type f -name "*.png"
    done
}

while IFS= read -r png; do
    case "$png" in
    *.new.png | *.diff.png | *.old.png) continue ;;
    esac

    base="${png%.png}"
    if [ -f "$base.new.png" ] || [ -f "$base.old.png" ]; then
        kept=$((kept + 1))
    else
        rm -f "$png"
        deleted=$((deleted + 1))
    fi
done < <(find_snapshot_pngs)

echo "Kept $kept changed snapshot(s), deleted $deleted unchanged baseline(s)."
