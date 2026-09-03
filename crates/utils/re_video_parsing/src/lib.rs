//! Bitstream utilities shared by [`re_video`](https://docs.rs/re_video) and
//! [`re_gpu_video`](https://docs.rs/re_gpu_video).

pub mod h264;
pub mod nalu;

pub use h264::{ParsedSps, SpsInfo, max_num_reorder_frames};

pub use nalu::{
    ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError, NotAnnexBError, nal_ranges,
    write_length_prefixed_nalus_to_annexb_stream,
};
