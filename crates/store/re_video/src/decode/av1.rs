//! AV1 support.

use std::sync::atomic::{AtomicBool, Ordering};

use dav1d::{PixelLayout, PlanarImageComponent};

use crate::Time;

use super::{
    Chunk, ColorPrimaries, Error, Frame, OutputCallback, PixelFormat, Result, SyncDecoder,
    YuvPixelLayout, YuvRange,
};

pub struct SyncDav1dDecoder {
    decoder: dav1d::Decoder,
}

impl SyncDav1dDecoder {
    pub fn new() -> Result<Self> {
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

        Ok(Self { decoder })
    }
}

impl SyncDecoder for SyncDav1dDecoder {
    fn submit_chunk(&mut self, should_stop: &AtomicBool, chunk: Chunk, on_output: &OutputCallback) {
        re_tracing::profile_function!();
        submit_chunk(&mut self.decoder, chunk, on_output);
        output_frames(should_stop, &mut self.decoder, on_output);
    }

    /// Clear and reset everything
    fn reset(&mut self) {
        re_tracing::profile_function!();

        self.decoder.flush();

        debug_assert!(matches!(self.decoder.get_picture(), Err(dav1d::Error::Again)),
            "There should be no pending pictures, since we output them directly after submitting a chunk.");
    }
}

fn submit_chunk(decoder: &mut dav1d::Decoder, chunk: Chunk, on_output: &OutputCallback) {
    re_tracing::profile_function!();
    econtext::econtext_function_data!(format!("chunk timestamp: {:?}", chunk.timestamp));

    re_tracing::profile_scope!("send_data");
    match decoder.send_data(
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
fn output_frames(
    should_stop: &AtomicBool,
    decoder: &mut dav1d::Decoder,
    on_output: &OutputCallback,
) -> usize {
    re_tracing::profile_function!();
    let mut count = 0;
    while !should_stop.load(Ordering::SeqCst) {
        let picture = {
            econtext::econtext!("get_picture");
            decoder.get_picture()
        };
        match picture {
            Ok(picture) => {
                output_picture(&picture, on_output);
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

fn output_picture(picture: &dav1d::Picture, on_output: &(dyn Fn(Result<Frame>) + Send + Sync)) {
    // TODO(jan): support other parameters?
    // What do these even do:
    // - matrix_coefficients
    // - transfer_characteristics

    let data = match picture.pixel_layout() {
        PixelLayout::I400 => picture.plane(PlanarImageComponent::Y).to_vec(),
        PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444 => {
            let mut data = Vec::with_capacity(
                picture.stride(PlanarImageComponent::Y) as usize
                    + picture.stride(PlanarImageComponent::U) as usize
                    + picture.stride(PlanarImageComponent::V) as usize,
            );
            data.extend_from_slice(&picture.plane(PlanarImageComponent::Y));
            data.extend_from_slice(&picture.plane(PlanarImageComponent::U));
            data.extend_from_slice(&picture.plane(PlanarImageComponent::V));
            data // TODO: how badly does this break with hdr?
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
        primaries: color_primaries(picture),
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

fn color_primaries(picture: &dav1d::Picture) -> ColorPrimaries {
    #[allow(clippy::match_same_arms)]
    match picture.color_primaries() {
        dav1d::pixel::ColorPrimaries::Reserved
        | dav1d::pixel::ColorPrimaries::Reserved0
        | dav1d::pixel::ColorPrimaries::Unspecified => {
            // This happens quite often. Don't issue a warning, that would be noise!

            if picture.transfer_characteristic() == dav1d::pixel::TransferCharacteristic::SRGB {
                // If the transfer characteristic is sRGB, assume BT.709 primaries, would be quite odd otherwise.
                // TODO(andreas): Other transfer characteristics may also hint at primaries.
                ColorPrimaries::Bt709
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
                // ... then again, eyeballing VLC it looks like it just always assumes BT.709.
                // The handwavy test case employed here was the same video in low & high resolution
                // without specified primaries. Both looked the same.
                ColorPrimaries::Bt709
            }
        }

        dav1d::pixel::ColorPrimaries::BT709 => ColorPrimaries::Bt709,

        // NTSC standard. Close enough to BT.601 for now. TODO(andreas): Is it worth warning?
        dav1d::pixel::ColorPrimaries::BT470M => ColorPrimaries::Bt601,

        // PAL standard. Close enough to BT.601 for now. TODO(andreas): Is it worth warning?
        dav1d::pixel::ColorPrimaries::BT470BG => ColorPrimaries::Bt601,

        // These are both using BT.2020 primaries.
        dav1d::pixel::ColorPrimaries::ST170M | dav1d::pixel::ColorPrimaries::ST240M => {
            ColorPrimaries::Bt601
        }

        // Is st428 also HDR? Not sure.
        // BT2020 and P3 variants definitely are ;)
        dav1d::pixel::ColorPrimaries::BT2020
        | dav1d::pixel::ColorPrimaries::ST428
        | dav1d::pixel::ColorPrimaries::P3DCI
        | dav1d::pixel::ColorPrimaries::P3Display => {
            // TODO(#7594): HDR support.
            re_log::warn_once!("Video specified HDR color primaries. Rerun doesn't handle HDR colors correctly yet. Color artifacts may be visible.");
            ColorPrimaries::Bt709
        }

        dav1d::pixel::ColorPrimaries::Film | dav1d::pixel::ColorPrimaries::Tech3213 => {
            re_log::warn_once!(
                "Video specified unsupported color primaries {:?}. Color artifacts may be visible.",
                picture.color_primaries()
            );
            ColorPrimaries::Bt709
        }
    }
}
