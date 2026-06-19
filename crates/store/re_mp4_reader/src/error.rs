/// Errors produced by the mp4 reader.
#[derive(Debug, thiserror::Error)]
pub enum Mp4Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("Chunk construction: {0}")]
    Chunk(#[from] re_chunk::ChunkError),

    #[error(
        "MP4 file is too large ({0} bytes). \
         Maximum supported blob size is ~2 GiB due to Arrow i32 offset limits."
    )]
    AssetTooLarge(usize),

    #[error("MP4 demux: {0}")]
    Demux(#[from] re_video::VideoLoadError),

    #[error(
        "MP4 contains B-frames; the `VideoStream` archetype does not yet model differing \
         DTS/PTS (see https://github.com/rerun-io/rerun/issues/10090). \
         Use `Mode::Asset` or pass `allow_b_frames = true` and transcode downstream."
    )]
    BFramesInStreamMode,

    #[error("MP4 with image-sequence codec is not supported by `Mode::Stream`; use `Mode::Asset`")]
    ImageSequenceInStreamMode,

    #[error(
        "MP4 has samples before the first keyframe; `Mode::Stream` requires the stream to begin \
         on a keyframe (a decoder cannot start mid-GOP). Use `Mode::Asset`."
    )]
    SamplesBeforeFirstKeyframe,

    #[error("MP4 has no timescale; cannot derive sample timestamps")]
    NoTimescale,

    #[error("MP4 sample conversion: {0}")]
    SampleConversion(String),
}
