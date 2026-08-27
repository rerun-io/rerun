#!/usr/bin/env bash
# Regenerates the AV1 parser test fixtures. Requires ffmpeg with libaom-av1.
#
# Each fixture is an IVF file, whose frames are the temporal units the golden
# trace tests push one at a time.
# After regenerating, the insta snapshots in ../../src/vulkan/av1/snapshots
# must be re-reviewed against a reference decoder, the traces encode encoder choices.
set -eu
cd "$(dirname "$0")"

SRC=(-f lavfi -i "testsrc2=size=64x64:rate=30")
OUT=(-pix_fmt yuv420p -c:v libaom-av1 -f ivf)
# One thread and no row multithreading keep the output stable across runs and machines.
DETERMINISTIC=(-cpu-used 8 -threads 1 -row-mt 0 -crf 50 -b:v 0)

# All frames are key frames.
ffmpeg -y "${SRC[@]}" -frames:v 8 "${OUT[@]}" "${DETERMINISTIC[@]}" \
    -g 1 i_only.ivf

# Key frame then inter frames only: no hidden frames, decoding order == output order.
ffmpeg -y "${SRC[@]}" -frames:v 16 "${OUT[@]}" "${DETERMINISTIC[@]}" \
    -g 30 -usage realtime -lag-in-frames 0 ippp.ivf

# Alternate reference frames: the encoder codes frames that are not shown when
# decoded and outputs them later with `show_existing_frame`.
ffmpeg -y "${SRC[@]}" -frames:v 24 "${OUT[@]}" "${DETERMINISTIC[@]}" \
    -g 30 -lag-in-frames 19 alt_ref.ivf

# Several tiles per frame, each with its own byte range in the temporal unit.
ffmpeg -y "${SRC[@]}" -frames:v 8 -s 256x256 "${OUT[@]}" "${DETERMINISTIC[@]}" \
    -g 30 -usage realtime -lag-in-frames 0 -tiles 2x2 multi_tile.ivf

# A second sequence at a different resolution, pushed after `ippp` by the test:
# a new sequence header the decoder has to rebuild its session for.
ffmpeg -y -f lavfi -i "testsrc2=size=96x64:rate=30" -frames:v 6 "${OUT[@]}" \
    "${DETERMINISTIC[@]}" -g 30 -usage realtime -lag-in-frames 0 seq_change.ivf
