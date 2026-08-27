//! The public decoder: access units in, GPU-texture frames in presentation order out.

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

/// Decodes access units of one codec into [`DecodedFrame`]s, in presentation order.
///
/// Created via [`crate::GpuVideoContext::create_decoder`].
/// Decoding blocks on the GPU work once enough frames are in flight, so this
/// belongs on a decoder worker thread, never on the render thread.
pub struct Decoder {
    inner: DecoderInner,
    sorter: ReorderBuffer<DecodedFrame>,
}

enum DecoderInner {
    Vulkan(crate::vulkan::TextureDecoder),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl Decoder {
    pub(crate) fn new_vulkan(decoder: crate::vulkan::TextureDecoder) -> Self {
        Self {
            inner: DecoderInner::Vulkan(decoder),
            sorter: ReorderBuffer::new(),
        }
    }

    /// Decodes one access unit, returning zero or more frames in presentation order.
    ///
    /// One access unit is one annex-b frame for H.264 and H.265, and one temporal
    /// unit of OBUs in low-overhead format for AV1. Decoding must start at a random
    /// access point carrying its parameter sets. `pts` travels into
    /// [`DecodedFrame::pts`] untouched. A frame comes out once its GPU work finished
    /// and its presentation order is settled, so it may take up to
    /// [`Self::reorder_delay`] further access units. Any error leaves the decoder
    /// waiting for the next random access point, like after [`Self::reset`].
    pub fn push_access_unit(
        &mut self,
        data: &[u8],
        pts: i64,
    ) -> Result<Vec<DecodedFrame>, DecodeError> {
        let mut out = Vec::new();
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                let sorted = decoder.emits_in_presentation_order();
                for (key, frame) in decoder.push_access_unit(data, pts)? {
                    if sorted {
                        out.push(frame);
                    } else {
                        self.sorter
                            .push(key, frame.is_idr, frame, reorder_delay, &mut out);
                    }
                }
            }
        }
        Ok(out)
    }

    /// Waits for the in-flight GPU work and emits the remaining buffered frames:
    /// the stream ended.
    pub fn flush(&mut self) -> Result<Vec<DecodedFrame>, DecodeError> {
        let mut out = Vec::new();
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                let sorted = decoder.emits_in_presentation_order();
                for (key, frame) in decoder.flush()? {
                    if sorted {
                        out.push(frame);
                    } else {
                        self.sorter
                            .push(key, frame.is_idr, frame, reorder_delay, &mut out);
                    }
                }
            }
        }
        self.sorter.flush(&mut out);
        Ok(out)
    }

    /// Drops all frame state for a seek. The next access unit must hold a random
    /// access point.
    pub fn reset(&mut self) {
        match &mut self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reset(),
        }
        self.sorter.reset();
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
        assert_send::<super::Decoder>();
        assert_send::<super::DecodedFrame>();
    }
}
