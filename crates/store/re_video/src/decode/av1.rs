//! AV1 support.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::Time;
use dav1d::{PixelLayout, PlanarImageComponent};

use super::{
    Chunk, Error, Frame, OutputCallback, PixelFormat, Result, SyncDecoder, YuvMatrixCoefficients,
    YuvPixelLayout, YuvRange,
};

pub struct SyncDav1dDecoder {
    decoder: dav1d::Decoder,
    debug_name: String,
}

impl SyncDecoder for SyncDav1dDecoder {
    fn submit_chunk(&mut self, should_stop: &AtomicBool, chunk: Chunk, on_output: &OutputCallback) {
        re_tracing::profile_function!();
        self.submit_chunk(chunk, on_output);
        self.output_frames(should_stop, on_output);
    }

    /// Clear and reset everything
    fn reset(&mut self) {
        re_tracing::profile_function!();

        self.decoder.flush();

        debug_assert!(matches!(self.decoder.get_picture(), Err(dav1d::Error::Again)),
            "There should be no pending pictures, since we output them directly after submitting a chunk.");
    }
}

impl SyncDav1dDecoder {
    pub fn new(debug_name: String) -> Result<Self> {
        re_tracing::profile_function!();

        // TODO(#7671): enable this warning again on Linux when the `nasm` feature actually does something
        #[allow(clippy::overly_complex_bool_expr)]
        if !cfg!(target_os = "linux") && !cfg!(feature = "nasm") {
            re_log::warn_once!(
                "NOTE: native AV1 video decoder is running extra slowly. \
                Speed it up by compiling Rerun with the `nasm` feature enabled. \
                You'll need to also install nasm: https://nasm.us/"
            );
        }

        // See https://videolan.videolan.me/dav1d/structDav1dSettings.html for settings docs
        let mut settings = dav1d::Settings::new();

        // Prioritize delivering video frames, not error messages.
        settings.set_strict_std_compliance(false);

        // Set to 1 for low-latency decoding.
        settings.set_max_frame_delay(1);

        let decoder = dav1d::Decoder::with_settings(&settings)?;

        Ok(Self {
            decoder,
            debug_name,
        })
    }

    fn submit_chunk(&mut self, chunk: Chunk, on_output: &OutputCallback) {
        re_tracing::profile_function!();
        econtext::econtext_function_data!(format!("chunk timestamp: {:?}", chunk.timestamp));

        re_tracing::profile_scope!("send_data");
        match self.decoder.send_data(
            chunk.data,
            None,
            Some(chunk.timestamp.0),
            Some(chunk.duration.0),
        ) {
            Ok(()) => {}
            Err(err) => {
                debug_assert!(err != dav1d::Error::Again, "Bug in AV1 decoder: send_data returned `Error::Again`. This shouldn't happen, since we process all images in a chunk right away");
                on_output(Err(Error::Dav1d(err)));
            }
        };
    }

    /// Returns the number of new frames.
    fn output_frames(&mut self, should_stop: &AtomicBool, on_output: &OutputCallback) -> usize {
        re_tracing::profile_function!();
        let mut count = 0;
        while !should_stop.load(Ordering::SeqCst) {
            let picture = {
                econtext::econtext!("get_picture");
                self.decoder.get_picture()
            };
            match picture {
                Ok(picture) => {
                    output_picture(&self.debug_name, &picture, on_output);
                    count += 1;
                }
                Err(dav1d::Error::Again) => {
                    // We need to submit more chunks to get more pictures
                    break;
                }
                Err(err) => {
                    on_output(Err(Error::Dav1d(err)));
                }
            }
        }
        count
    }
}

fn output_picture(
    debug_name: &str,
    picture: &dav1d::Picture,
    on_output: &(dyn Fn(Result<Frame>) + Send + Sync),
) {
    let data = {
        re_tracing::profile_scope!("copy_picture_data");

        match picture.pixel_layout() {
            PixelLayout::I400 => picture.plane(PlanarImageComponent::Y).to_vec(),
            PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444 => {
                // TODO(#7594): If `picture.bit_depth()` isn't 8 we have a problem:
                // We can't handle high bit depths yet and the YUV converter at the other side
                // bases its opinion on what an acceptable number of incoming bytes is on this.
                // So we just clamp to that expectation, ignoring `picture.stride(PlanarImageComponent::Y)` & friends.
                // Note that `bit_depth` is either 8 or 16, which is semi-independent `bits_per_component` (which is None/8/10/12).
                if picture.bit_depth() != 8 {
                    re_log::warn_once!(
                        "Video {debug_name:?} uses {} bis per component. Only a bit depth of 8 bit is currently unsupported.",
                        picture.bits_per_component().map_or(picture.bit_depth(), |bpc| bpc.0)
                    );
                }

                let height_y = picture.height() as usize;
                let height_uv = match picture.pixel_layout() {
                    PixelLayout::I400 => 0,
                    PixelLayout::I420 => height_y / 2,
                    PixelLayout::I422 | PixelLayout::I444 => height_y,
                };

                let packed_stride_y = picture.width() as usize;
                let actual_stride_y = picture.stride(PlanarImageComponent::Y) as usize;

                let packed_stride_uv = match picture.pixel_layout() {
                    PixelLayout::I400 => 0,
                    PixelLayout::I420 | PixelLayout::I422 => packed_stride_y / 2,
                    PixelLayout::I444 => packed_stride_y,
                };
                let actual_stride_uv = picture.stride(PlanarImageComponent::U) as usize; // U / V stride is always the same.

                let num_packed_bytes_y = packed_stride_y * height_y;
                let num_packed_bytes_uv = packed_stride_uv * height_uv;

                if actual_stride_y == packed_stride_y && actual_stride_uv == packed_stride_uv {
                    // Best case scenario: There's no additional strides at all, so we can just copy the data directly.
                    // TODO(andreas): This still has *significant* overhead for 8k video. Can we take ownership of the data instead without a copy?
                    re_tracing::profile_scope!("fast path");
                    let plane_y = &picture.plane(PlanarImageComponent::Y)[0..num_packed_bytes_y];
                    let plane_u = &picture.plane(PlanarImageComponent::U)[0..num_packed_bytes_uv];
                    let plane_v = &picture.plane(PlanarImageComponent::V)[0..num_packed_bytes_uv];
                    [plane_y, plane_u, plane_v].concat()
                } else {
                    // At least either y or u/v have strides.
                    //
                    // We could make our image ingestion pipeline even more sophisticated and pass that stride information through.
                    // But given that this is a matter of replacing a single large memcpy with a few hundred _still_ quite large ones,
                    // this should not make a lot of difference (citation needed!).

                    let mut data = Vec::with_capacity(num_packed_bytes_y + num_packed_bytes_uv * 2);
                    {
                        let plane = picture.plane(PlanarImageComponent::Y);
                        if packed_stride_y == actual_stride_y {
                            data.extend_from_slice(&plane[0..num_packed_bytes_y]);
                        } else {
                            re_tracing::profile_scope!("slow path, y-plane");

                            for y in 0..height_y {
                                let offset = y * actual_stride_y;
                                data.extend_from_slice(&plane[offset..(offset + packed_stride_y)]);
                            }
                        }
                    }
                    for comp in [PlanarImageComponent::U, PlanarImageComponent::V] {
                        let plane = picture.plane(comp);
                        if actual_stride_uv == packed_stride_uv {
                            data.extend_from_slice(&plane[0..num_packed_bytes_uv]);
                        } else {
                            re_tracing::profile_scope!("slow path, u/v-plane");

                            for y in 0..height_uv {
                                let offset = y * actual_stride_uv;
                                data.extend_from_slice(&plane[offset..(offset + packed_stride_uv)]);
                            }
                        }
                    }

                    data
                }
            }
        }
    };

    let format = PixelFormat::Yuv {
        layout: match picture.pixel_layout() {
            PixelLayout::I400 => YuvPixelLayout::Y400,
            PixelLayout::I420 => YuvPixelLayout::Y_U_V420,
            PixelLayout::I422 => YuvPixelLayout::Y_U_V422,
            PixelLayout::I444 => YuvPixelLayout::Y_U_V444,
        },
        range: match picture.color_range() {
            dav1d::pixel::YUVRange::Limited => YuvRange::Limited,
            dav1d::pixel::YUVRange::Full => YuvRange::Full,
        },
        coefficients: color_primaries(debug_name, picture),
    };

    let frame = Frame {
        data,
        width: picture.width(),
        height: picture.height(),
        format,
        timestamp: Time(picture.timestamp().unwrap_or(0)),
        duration: Time(picture.duration()),
    };
    on_output(Ok(frame));
}

fn color_primaries(debug_name: &str, picture: &dav1d::Picture) -> YuvMatrixCoefficients {
    #[allow(clippy::match_same_arms)]
    match picture.color_primaries() {
        dav1d::pixel::ColorPrimaries::Reserved
        | dav1d::pixel::ColorPrimaries::Reserved0
        | dav1d::pixel::ColorPrimaries::Unspecified => {
            // This happens quite often. Don't issue a warning, that would be noise!

            if picture.transfer_characteristic() == dav1d::pixel::TransferCharacteristic::SRGB {
                // If the transfer characteristic is sRGB, assume BT.709 primaries, would be quite odd otherwise.
                // TODO(andreas): Other transfer characteristics may also hint at primaries.
                YuvMatrixCoefficients::Bt709
            } else {
                // Best guess: If the picture is 720p+ assume Bt709 because Rec709
                // is the "HDR" standard.
                // TODO(#7594): 4k/UHD material should probably assume Bt2020?
                // else if picture.height() >= 720 {
                //     ColorPrimaries::Bt709
                // } else {
                //     ColorPrimaries::Bt601
                // }
                //
                // This is also what the mpv player does (and probably others):
                // https://wiki.x266.mov/docs/colorimetry/primaries#2-unspecified
                //
                // …then again, eyeballing VLC it looks like it just always assumes BT.709.
                // The handwavy test case employed here was the same video in low & high resolution
                // without specified primaries. Both looked the same.
                YuvMatrixCoefficients::Bt709
            }
        }

        dav1d::pixel::ColorPrimaries::BT709 => YuvMatrixCoefficients::Bt709,

        // NTSC standard. Close enough to BT.601 for now. TODO(andreas): Is it worth warning?
        dav1d::pixel::ColorPrimaries::BT470M => YuvMatrixCoefficients::Bt601,

        // PAL standard. Close enough to BT.601 for now. TODO(andreas): Is it worth warning?
        dav1d::pixel::ColorPrimaries::BT470BG => YuvMatrixCoefficients::Bt601,

        // These are both using BT.2020 primaries.
        dav1d::pixel::ColorPrimaries::ST170M | dav1d::pixel::ColorPrimaries::ST240M => {
            YuvMatrixCoefficients::Bt601
        }

        // Is st428 also HDR? Not sure.
        // BT2020 and P3 variants definitely are ;)
        dav1d::pixel::ColorPrimaries::BT2020
        | dav1d::pixel::ColorPrimaries::ST428
        | dav1d::pixel::ColorPrimaries::P3DCI
        | dav1d::pixel::ColorPrimaries::P3Display => {
            // TODO(#7594): HDR support.
            re_log::warn_once!("Video {debug_name:?} specified HDR color primaries. Rerun doesn't handle HDR colors correctly yet. Color artifacts may be visible.");
            YuvMatrixCoefficients::Bt709
        }

        dav1d::pixel::ColorPrimaries::Film | dav1d::pixel::ColorPrimaries::Tech3213 => {
            re_log::warn_once!(
                "Video {debug_name:?} specified unsupported color primaries {:?}. Color artifacts may be visible.",
                picture.color_primaries()
            );
            YuvMatrixCoefficients::Bt709
        }
    }
}
