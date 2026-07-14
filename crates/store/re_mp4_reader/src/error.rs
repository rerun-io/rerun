use re_sdk_types::components::VideoCodec;

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
        "MP4 contains B-frames; stripping them requires FFmpeg, which is not \
         available in this build. Use `Mode::Asset` instead."
    )]
    BFramesRequireFfmpeg,

    #[error(
        "MP4 contains B-frames; stripping them with FFmpeg requires a seekable file, but this \
         stream was provided as in-memory bytes. Load it from a file path (`load_mp4`), or use \
         `Mode::Asset`."
    )]
    BFramesFromInMemoryBytes,

    #[error("Failed to strip B-frames from MP4 stream via FFmpeg: {0}")]
    Transcode(String),

    #[error(
        "MP4 stream uses codec {codec:?}, whose B-frames (DTS != PTS) the `VideoStream` \
         archetype cannot yet model (see https://github.com/rerun-io/rerun/issues/10090). \
         Only H.264 and H.265 B-frame sources can be transcoded; use `Mode::Asset` instead."
    )]
    BFramesUnsupportedCodec { codec: VideoCodec },

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
