//! The public decoder: H.264 access units in, GPU-texture frames in presentation order out.

use crate::{ColorProperties, DecodeError, sorter::ReorderBuffer};

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
    /// The plane views can be slightly larger: the texture pads odd display sizes
    /// to even ones, the excess row/column is not meant to be shown.
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
/// Decoding blocks on the GPU work, so this belongs on a decoder worker thread,
/// never on the render thread.
pub struct H264Decoder {
    inner: DecoderInner,
    sorter: ReorderBuffer<DecodedFrame>,
}

enum DecoderInner {
    Vulkan(crate::vulkan::TextureDecoder),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl H264Decoder {
    pub(crate) fn new_vulkan(decoder: crate::vulkan::TextureDecoder) -> Self {
        Self {
            inner: DecoderInner::Vulkan(decoder),
            sorter: ReorderBuffer::new(),
        }
    }

    /// Decodes one annex-b access unit, returning zero or more frames in presentation order.
    ///
    /// Decoding must start at an IDR frame carrying its SPS/PPS. `pts` travels into
    /// [`DecodedFrame::pts`] untouched. Any error leaves the decoder waiting for the
    /// next IDR frame, like after [`Self::reset`].
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
                    self.sorter
                        .push(key, frame.is_idr, frame, reorder_delay, &mut out);
                }
            }
        }
        Ok(out)
    }

    /// Emits the remaining buffered frames: the stream ended.
    pub fn flush(&mut self) -> Vec<DecodedFrame> {
        let mut out = Vec::new();
        self.sorter.flush(&mut out);
        out
    }

    /// Drops all frame state for a seek. The next access unit must hold an IDR frame.
    pub fn reset(&mut self) {
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reset(),
        }
        self.sorter.reset();
    }

    /// How many frames may need to be pushed beyond a frame before it comes out:
    /// `max_num_reorder_frames` of the active SPS.
    pub fn reorder_delay(&self) -> usize {
        match &self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reorder_delay(),
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
