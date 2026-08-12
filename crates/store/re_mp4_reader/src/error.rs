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
        "Transcoding this MP4 stream (B-frame stripping or a requested transform) requires \
         FFmpeg, which is not available in this build. Use `Mode::Asset` instead."
    )]
    TranscodeRequiresFfmpeg,

    #[error(
        "Transcoding this MP4 stream with FFmpeg requires a seekable file, but this stream was \
         provided as in-memory bytes. Load it from a file path (`load_mp4`), or use `Mode::Asset`."
    )]
    TranscodeRequiresSeekableFile,

    #[error("Failed to transcode MP4 stream via FFmpeg: {0}")]
    Transcode(String),

    #[error(
        "This FFmpeg build has no encoder for output codec {codec:?}. Install a build with the \
         matching encoder (e.g. libvpx for VP8/VP9, libsvtav1/libaom for AV1), or choose a \
         different `output_codec`."
    )]
    NoEncoderAvailable { codec: re_video::VideoCodec },

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
