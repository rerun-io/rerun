//! The public decoder: H.264 access units in, GPU-texture frames in presentation order out.

use crate::{ColorProperties, DecodeError, reorder::ReorderBuffer};

/// One decoded frame living in a `wgpu` texture, exposed as its two NV12 plane views.
///
/// The texture stays alive for as long as the frame (or one of its views) does,
/// and is recycled by the decoder afterwards.
pub struct DecodedFrame {
    /// The luma plane, an `R8Unorm` view.
    pub y: wgpu::TextureView,

    /// The interleaved chroma plane at half extent, an `Rg8Unorm` view.
    pub uv: wgpu::TextureView,

    /// Display width in luma texels.
    ///
    /// The plane views can be one texel larger, since the texture pads odd display
    /// sizes to even ones. The padding row/column is not meant to be shown.
    pub width: u32,

    /// Display height in luma texels.
    pub height: u32,

    /// The caller's timestamp from the access unit that produced this frame,
    /// passed through untouched.
    pub pts: i64,

    pub is_idr: bool,

    pub color: ColorProperties,

    /// The owning texture, kept alive with the frame.
    _texture: wgpu::Texture,
}

impl DecodedFrame {
    pub(crate) fn new(
        texture: wgpu::Texture,
        y: wgpu::TextureView,
        uv: wgpu::TextureView,
        width: u32,
        height: u32,
        pts: i64,
        is_idr: bool,
        color: ColorProperties,
    ) -> Self {
        Self {
            y,
            uv,
            width,
            height,
            pts,
            is_idr,
            color,
            _texture: texture,
        }
    }
}

/// Decodes H.264 access units into [`DecodedFrame`]s, in presentation order.
///
/// Created via [`crate::GpuVideoContext::create_h264_decoder`].
/// Decoding blocks on the GPU work once enough frames are in flight, so this
/// belongs on a decoder worker thread, never on the render thread.
pub struct H264Decoder {
    inner: DecoderInner,
    reorder: ReorderBuffer<DecodedFrame>,
}

enum DecoderInner {
    Vulkan(crate::vulkan::TextureDecoder),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl H264Decoder {
    pub(crate) fn new_vulkan(decoder: crate::vulkan::TextureDecoder) -> Self {
        Self {
            inner: DecoderInner::Vulkan(decoder),
            reorder: ReorderBuffer::new(),
        }
    }

    /// Makes an SPS the decoder's active one, ahead of the stream's own copy of it.
    ///
    /// Streams repeat their SPS in front of every IDR frame. Handing over the SPS that
    /// the demuxer already parsed lets the decoder recognize those repeats by their
    /// bytes rather than parsing them again. Rejected the same way a stream's own SPS
    /// is when the device can't decode it.
    pub fn preset_sps(
        &mut self,
        sps: std::sync::Arc<re_video_parsing::ParsedSps>,
    ) -> Result<(), DecodeError> {
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => decoder.preset_sps(sps),
        }
    }

    /// Decodes one annex-b access unit, returning zero or more frames in presentation order.
    ///
    /// Decoding must start at an IDR frame carrying its SPS/PPS. `pts` travels into
    /// [`DecodedFrame::pts`] untouched. A frame comes out once its GPU work finished
    /// and its presentation order is settled, so it may take up to
    /// [`Self::reorder_delay`] further access units. Any error leaves the decoder
    /// waiting for the next IDR frame, like after [`Self::reset`].
    pub fn push_access_unit(
        &mut self,
        data: &[u8],
        pts: i64,
    ) -> Result<Vec<DecodedFrame>, DecodeError> {
        let mut out = Vec::new();
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                for (key, frame) in decoder.push_access_unit(data, pts)? {
                    self.reorder
                        .push(key, frame.is_idr, frame, reorder_delay, &mut out);
                }
            }
        }
        Ok(out)
    }

    /// Waits for the in-flight GPU work and returns the remaining buffered frames.
    ///
    /// Call this once the stream ended.
    pub fn flush(&mut self) -> Result<Vec<DecodedFrame>, DecodeError> {
        let mut out = Vec::new();
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                for (key, frame) in decoder.flush()? {
                    self.reorder
                        .push(key, frame.is_idr, frame, reorder_delay, &mut out);
                }
            }
        }
        self.reorder.flush(&mut out);
        Ok(out)
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    pub fn reset(&mut self) {
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reset(),
        }
        self.reorder.reset();
    }

    /// How many frames may need to be pushed beyond a frame before it comes out:
    /// `max_num_reorder_frames` of the active SPS, plus the frames the backend
    /// may keep in flight on the GPU.
    pub fn reorder_delay(&self) -> usize {
        match &self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reorder_delay() + decoder.pipeline_depth(),
        }
    }
}

#[cfg(test)]
mod tests {
    /// The decoder runs on a worker thread and its frames cross a channel.
    #[test]
    fn decoder_and_frames_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<super::H264Decoder>();
        assert_send::<super::DecodedFrame>();
    }
}
