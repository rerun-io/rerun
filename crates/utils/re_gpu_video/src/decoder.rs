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

    /// The owning textures, kept alive with the frame: one NV12 texture holding both
    /// planes on Vulkan, one texture per plane on `VideoToolbox`.
    _textures: Vec<wgpu::Texture>,
}

impl DecodedFrame {
    /// A frame whose planes are two views of one NV12 texture.
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
            _textures: vec![texture],
        }
    }

    /// A frame whose planes live in a texture each.
    #[cfg(target_os = "macos")]
    #[expect(clippy::too_many_arguments)]
    pub(crate) fn new_planar(
        y_texture: wgpu::Texture,
        uv_texture: wgpu::Texture,
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
            _textures: vec![y_texture, uv_texture],
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
    Vulkan(Box<crate::vulkan::TextureDecoder>),

    #[cfg(target_os = "macos")]
    VideoToolbox(Box<crate::videotoolbox::VideoToolboxDecoder>),
}

impl Decoder {
    pub(crate) fn new_vulkan(decoder: crate::vulkan::TextureDecoder) -> Self {
        Self {
            inner: DecoderInner::Vulkan(Box::new(decoder)),
            sorter: ReorderBuffer::new(),
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn new_video_toolbox(decoder: crate::videotoolbox::VideoToolboxDecoder) -> Self {
        Self {
            inner: DecoderInner::VideoToolbox(Box::new(decoder)),
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
                    Self::sort(
                        &mut self.sorter,
                        key,
                        frame,
                        reorder_delay,
                        sorted,
                        &mut out,
                    );
                }
            }

            #[cfg(target_os = "macos")]
            DecoderInner::VideoToolbox(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                for (key, frame) in decoder.push_access_unit(data, pts)? {
                    Self::sort(&mut self.sorter, key, frame, reorder_delay, false, &mut out);
                }
            }
        }
        Ok(out)
    }

    /// Either passes a frame straight through, or buffers it until its place in
    /// presentation order is settled.
    fn sort(
        sorter: &mut ReorderBuffer<DecodedFrame>,
        key: i64,
        frame: DecodedFrame,
        reorder_delay: usize,
        already_sorted: bool,
        out: &mut Vec<DecodedFrame>,
    ) {
        if already_sorted {
            out.push(frame);
        } else {
            sorter.push(key, frame.is_idr, frame, reorder_delay, out);
        }
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
                    Self::sort(
                        &mut self.sorter,
                        key,
                        frame,
                        reorder_delay,
                        sorted,
                        &mut out,
                    );
                }
            }

            #[cfg(target_os = "macos")]
            DecoderInner::VideoToolbox(decoder) => {
                let reorder_delay = decoder.reorder_delay();
                for (key, frame) in decoder.flush()? {
                    Self::sort(&mut self.sorter, key, frame, reorder_delay, false, &mut out);
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

            #[cfg(target_os = "macos")]
            DecoderInner::VideoToolbox(decoder) => decoder.reset(),
        }
        self.sorter.reset();
    }

    /// How many frames may need to be pushed beyond a frame before it comes out:
    /// `max_num_reorder_frames` of the active SPS, plus the frames the backend
    /// may keep in flight on the GPU.
    pub fn reorder_delay(&self) -> usize {
        match &self.inner {
            DecoderInner::Vulkan(decoder) => decoder.reorder_delay() + decoder.pipeline_depth(),

            #[cfg(target_os = "macos")]
            DecoderInner::VideoToolbox(decoder) => {
                decoder.reorder_delay() + decoder.pipeline_depth()
            }
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
