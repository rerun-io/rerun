#!/usr/bin/env bash
# Regenerates the H.264 parser test fixtures. Requires ffmpeg with libx264.
#
# Each fixture is a raw annex-b elementary stream with access unit delimiters,
# which the golden-trace tests split the stream into access units by.
# After regenerating, the insta snapshots in ../../src/vulkan/h264/snapshots
# must be re-reviewed against a reference decoder, the traces encode encoder choices.
set -eu
cd "$(dirname "$0")"

SRC=(-f lavfi -i "testsrc2=size=64x64:rate=30")
OUT=(-pix_fmt yuv420p -c:v libx264 -qp 35 -bsf:v h264_metadata=aud=insert -f h264)
DETERMINISTIC="threads=1:sliced-threads=0:rc-lookahead=8"

# All frames are IDR.
ffmpeg -y "${SRC[@]}" -frames:v 8 "${OUT[@]}" -g 1 -x264-params "$DETERMINISTIC" i_only.h264

# I then P frames only: POC type 2, decoding order == presentation order.
ffmpeg -y "${SRC[@]}" -frames:v 16 "${OUT[@]}" -bf 0 -g 30 -x264-params "$DETERMINISTIC" ippp.h264

# B frames without B references: POC type 0 with reordering.
ffmpeg -y "${SRC[@]}" -frames:v 16 "${OUT[@]}" -bf 2 -g 30 \
    -x264-params "$DETERMINISTIC:b-adapt=0:b-pyramid=none" ipb.h264

# B pyramid: B frames used as references.
ffmpeg -y "${SRC[@]}" -frames:v 24 "${OUT[@]}" -bf 3 -g 30 \
    -x264-params "$DETERMINISTIC:b-adapt=0:b-pyramid=normal" ipb_pyramid.h264

# Several slices per frame.
ffmpeg -y "${SRC[@]}" -frames:v 8 "${OUT[@]}" -bf 0 -g 30 \
    -x264-params "$DETERMINISTIC:slices=3" multi_slice.h264

# Resolution change mid-stream: two concatenated encodes, the second opens
# with an IDR frame and an SPS with different dimensions.
ffmpeg -y "${SRC[@]}" -frames:v 6 "${OUT[@]}" -bf 0 -g 30 -x264-params "$DETERMINISTIC" /tmp/sps_change_a.h264
ffmpeg -y -f lavfi -i "testsrc2=size=96x64:rate=30" -frames:v 6 "${OUT[@]}" -bf 0 -g 30 \
    -x264-params "$DETERMINISTIC" /tmp/sps_change_b.h264
cat /tmp/sps_change_a.h264 /tmp/sps_change_b.h264 > sps_change.h264
rm /tmp/sps_change_a.h264 /tmp/sps_change_b.h264
