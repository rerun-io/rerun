use crate::{
    video::{DecodingError, FrameDecodingResult},
    RenderContext,
};

use super::VideoDecoder;

/// A [`VideoDecoder`] that always fails with [`DecodingError::NoNativeSupport`]
#[derive(Default)]
pub struct NoNativeVideoDecoder {}

impl VideoDecoder for NoNativeVideoDecoder {
    #[allow(clippy::unused_self)]
    fn frame_at(
        &mut self,
        _render_ctx: &RenderContext,
        _presentation_timestamp_s: f64,
    ) -> FrameDecodingResult {
        Err(DecodingError::NoNativeSupport)
    }
}
