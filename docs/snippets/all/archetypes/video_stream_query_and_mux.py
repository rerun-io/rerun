"""Query video stream from a recording and mux it to an mp4 video file."""

import argparse
import io
import sys
from fractions import Fraction

import av
import rerun as rr
from pyarrow import ChunkedArray


def read_h264_samples_from_rrd(rrd_path: str, video_entity: str, timeline: str) -> tuple[ChunkedArray, ChunkedArray]:
    """Load recording data and query video stream."""

    server = rr.server.Server(datasets={"video_stream": [rrd_path]})
    client = server.client()
    dataset = client.get_dataset("video_stream")
    df = dataset.filter_contents(video_entity).reader(index=timeline)

    # Make sure this is H.264 encoded.
    # For that we just read out the first codec value batch and check whether it's H.264.
    first_codec_batch = df.select(f"/{video_entity}:VideoStream:codec").execute_stream().next()
    if first_codec_batch is None:
        raise ValueError(f"There's no video stream codec specified at {video_entity} for timeline {timeline}.")
    codec_value = first_codec_batch.to_pyarrow().column(0)[0][0].as_py()
    if codec_value != rr.VideoCodec.H264.value:
        raise ValueError(
            f"Video stream codec is not H.264 at {video_entity} for timeline {timeline}. "
            f"Got {hex(codec_value)}, but the value for H.264 is {hex(rr.VideoCodec.H264.value)}."
        )
    else:
        print(f"Video stream codec is H.264 at {video_entity} for timeline {timeline}.")

    # Get the video stream
    timestamps_and_samples = df.select(timeline, f"/{video_entity}:VideoStream:sample").to_arrow_table()
    times = timestamps_and_samples[0]
    samples = timestamps_and_samples[1]

    print(f"Retrieved {len(samples)} video samples.")

    return times, samples


def mux_h264_to_mp4(times: ChunkedArray, samples: ChunkedArray, output_path: str) -> None:
    """Mux H.264 Annex B samples to an mp4 file using PyAV."""
    # See https://pyav.basswood-io.com/docs/stable/cookbook/basics.html#remuxing

    # Flatten out sample list into a single byte buffer.
    sample_bytes = samples.combine_chunks().flatten(recursive=True)
    sample_bytes = io.BytesIO(sample_bytes.buffers()[1])

    # Setup samples as input container.
    input_container = av.open(sample_bytes, mode="r", format="h264")  # Input is AnnexB H.264 stream.
    input_stream = input_container.streams.video[0]

    # Setup output container.
    output_container = av.open(output_path, mode="w")
    output_stream = output_container.add_stream_from_template(input_stream)

    # Timestamps are made relative to the first timestamp.
    start_time = times.chunk(0)[0]
    print(f"Offsetting timestamps with start time: {start_time}")

    # Demux and mux packets.
    for packet, time in zip(input_container.demux(input_stream), times, strict=False):
        packet.time_base = Fraction(1, 1_000_000_000)  # Assuming duration timestamps in nanoseconds.
        packet.pts = int(time.value - start_time.value)
        packet.dts = packet.pts  # dts == pts since there's no B-frames.
        packet.stream = output_stream
        output_container.mux(packet)

    input_container.close()
    output_container.close()


def main() -> None:
    parser = argparse.ArgumentParser(description="Query video stream from a recording and mux it to an mp4 video file.")
    parser.add_argument("input_rrd", type=str, help="Path to the input .rrd recording file")
    parser.add_argument(
        "-o", "--output", type=str, default="output.mp4", help="Output mp4 file path (default: output.mp4)"
    )
    parser.add_argument(
        "--entity", type=str, default="video_stream", help="Video entity path to query (default: video_stream)"
    )
    parser.add_argument("--timeline", type=str, default="time", help="Name of the timeline to query (default: time)")
    args = parser.parse_args()

    # Load recording data
    print(f"Loading recording from: {args.input_rrd}")
    times, samples = read_h264_samples_from_rrd(args.input_rrd, args.entity, args.timeline)

    print(f"Creating video file: {args.output}")
    mux_h264_to_mp4(times, samples, args.output)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nOperation cancelled by user.")
        sys.exit(0)
