mod mp4;

#[derive(Clone)]
pub struct Video {
    pub config: Config,

    /// How many time units per second there are.
    pub timescale: u64,

    /// Duration of the video, in time units.
    pub duration: u64,

    /// We split video into segments, each beginning with a key frame,
    /// followed by any number of delta frames.
    pub segments: Box<[Segment]>,

    /// This array stores all data used by samples.
    pub data: Box<[u8]>,
}

#[derive(Clone)]
pub struct Segment {
    /// Time of the first sample in this segment, in time units.
    pub timestamp: u64,

    /// List of samples contained in this segment.
    /// At least one sample per segment is guaranteed,
    /// and the first sample is always a key frame.
    pub samples: Box<[Sample]>,
}

#[derive(Debug, Clone)]
pub struct Sample {
    /// Time at which this sample appears, in time units.
    pub timestamp: u64,

    /// Offset into [`Video::data`]
    pub byte_offset: u32,

    /// Length of sample starting at [`Sample::byte_offset`].
    pub byte_length: u32,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// String used to identify the codec and some of its configuration.
    pub codec: Box<str>,

    /// Codec-specific configuration.
    pub description: Box<[u8]>,

    /// Natural height of the video.
    pub coded_height: u16,

    /// Natural width of the video.
    pub coded_width: u16,
}

#[derive(Debug)]
pub enum VideoLoadError {
    ParseMp4(::mp4::Error),
    NoVideoTrack,
    InvalidConfigFormat,
    InvalidSamples,
}

impl std::fmt::Display for VideoLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseMp4(err) => write!(f, "failed to parse video: {err}"),
            Self::NoVideoTrack => write!(f, "video file has no video tracks"),
            Self::InvalidConfigFormat => write!(f, "video file track config is invalid"),
            Self::InvalidSamples => write!(f, "video file has invalid sample entries"),
        }
    }
}

impl std::error::Error for VideoLoadError {}

impl From<::mp4::Error> for VideoLoadError {
    fn from(value: ::mp4::Error) -> Self {
        Self::ParseMp4(value)
    }
}

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video")
            .field("config", &self.config)
            .field("duration", &self.duration)
            .field("segments", &self.segments)
            .field("data", &self.data.len())
            .finish()
    }
}

impl std::fmt::Debug for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("timestamp", &self.timestamp)
            .field("samples", &self.samples.len())
            .finish()
    }
}
