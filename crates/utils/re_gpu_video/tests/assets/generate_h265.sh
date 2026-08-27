#!/usr/bin/env bash
# Regenerates the H.265 parser test fixtures. Requires ffmpeg with libx265.
#
# Each fixture is a raw annex-b elementary stream with access unit delimiters,
# which the golden-trace tests split the stream into access units by.
# After regenerating, the insta snapshots in ../../src/vulkan/h265/snapshots
# must be re-reviewed against a reference decoder, the traces encode encoder choices.
set -eu
cd "$(dirname "$0")"

SRC=(-f lavfi -i "testsrc2=size=64x64:rate=30")
OUT=(-pix_fmt yuv420p -c:v libx265 -f hevc)
# `aud=1` makes x265 emit the access unit delimiters the tests split on.
# One frame thread and no lookahead keep the output stable across runs and machines.
DETERMINISTIC="aud=1:frame-threads=1:pools=none:wpp=0:rc-lookahead=8:log-level=error"

# All frames are random access points.
ffmpeg -y "${SRC[@]}" -frames:v 8 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=1" i_only.h265

# I then P frames only: decoding order == presentation order.
ffmpeg -y "${SRC[@]}" -frames:v 16 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=30:bframes=0" ippp.h265

# B frames without B references: reordering, but a flat reference structure.
ffmpeg -y "${SRC[@]}" -frames:v 16 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=30:bframes=2:b-adapt=0:b-pyramid=0" ipb.h265

# B pyramid: B frames used as references, the deepest reference structure x265 emits.
ffmpeg -y "${SRC[@]}" -frames:v 24 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=30:bframes=3:b-adapt=0:b-pyramid=1" ipb_pyramid.h265

# Several slices per picture. x265 only emits those with wavefront parallel
# processing on, which also sets the PPS entropy coding sync flag.
ffmpeg -y "${SRC[@]}" -frames:v 8 "${OUT[@]}" -s 128x128 \
    -x265-params "${DETERMINISTIC/:wpp=0/}:qp=35:keyint=30:bframes=0:wpp=1:slices=2" \
    multi_slice.h265

# Resolution change mid-stream: two concatenated encodes, the second opens
# with a random access point and an SPS with different dimensions.
ffmpeg -y "${SRC[@]}" -frames:v 6 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=30:bframes=0" /tmp/sps_change_a.h265
ffmpeg -y -f lavfi -i "testsrc2=size=96x64:rate=30" -frames:v 6 "${OUT[@]}" \
    -x265-params "$DETERMINISTIC:qp=35:keyint=30:bframes=0" /tmp/sps_change_b.h265
cat /tmp/sps_change_a.h265 /tmp/sps_change_b.h265 > sps_change.h265
rm /tmp/sps_change_a.h265 /tmp/sps_change_b.h265
