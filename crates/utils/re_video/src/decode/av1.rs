//! AV1 support.

use std::sync::atomic::{AtomicBool, Ordering};

use dav1d::{PixelLayout, PlanarImageComponent};

use super::async_decoder_wrapper::SyncDecoder;
use super::{
    Chunk, DecodeError, Frame, FrameContent, FrameInfo, PixelFormat, Result, YuvMatrixCoefficients,
    YuvPixelLayout, YuvRange,
};
use crate::decode::FrameResult;
use crate::{Time, VideoDataDescription};

pub struct SyncDav1dDecoder {
    decoder: dav1d::Decoder,
    debug_name: String,
}

impl SyncDecoder for SyncDav1dDecoder {
    fn submit_chunk(
        &mut self,
        should_stop: &AtomicBool,
        chunk: Chunk,
        output_sender: &crossbeam::channel::Sender<FrameResult>,
    ) {
        re_tracing::profile_function!();
        self.submit_chunk(chunk, output_sender);
        self.output_frames(should_stop, output_sender);
    }

    /// Clear and reset everything
    fn reset(&mut self, _video_data_description: &VideoDataDescription) {
        re_tracing::profile_function!();

        self.decoder.flush();

        debug_assert!(
            matches!(self.decoder.get_picture(), Err(dav1d::Error::Again)),
            "There should be no pending pictures, since we output them directly after submitting a chunk."
        );
    }
}

impl SyncDav1dDecoder {
    pub fn new(debug_name: String) -> Result<Self> {
        re_tracing::profile_function!();

        if !cfg!(feature = "nasm") {
            // The `nasm` feature makes AV1 decoding much faster.
            // On Linux the difference is huge (~25x).
            // On Windows, the difference was also pretty big (unsure how big).
            // On an M3 Mac the difference is smaller (2-3x),
            // and even without `nasm` emilk can play an 8k video at 2x speed.

            if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
                re_log::warn_once!(
                    "The native AV1 video decoder is unnecessarily slow. \
                    Speed it up by compiling Rerun with the `nasm` feature enabled."
                );
            } else {
                // Better to return an error than to be perceived as being slow
                return Err(DecodeError::Dav1dWithoutNasm);
            }
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

    fn submit_chunk(
        &mut self,
        chunk: Chunk,
        output_sender: &crossbeam::channel::Sender<FrameResult>,
    ) {
        re_tracing::profile_function!();
        econtext::econtext_function_data!(format!(
            "chunk timestamp: {:?}",
            chunk.presentation_timestamp
        ));

        re_tracing::profile_scope!("send_data");
        match self.decoder.send_data(
            chunk.data,
            None,
            Some(chunk.presentation_timestamp.0),
            chunk.duration.map(|d| d.0),
        ) {
            Ok(()) => {}
            Err(err) => {
                debug_assert!(
                    err != dav1d::Error::Again,
                    "Bug in AV1 decoder: send_data returned `Error::Again`. This shouldn't happen, since we process all images in a chunk right away"
                );
                output_sender.send(Err(DecodeError::Dav1d(err))).ok();
            }
        }
    }

    /// Returns the number of new frames.
    fn output_frames(
        &mut self,
        should_stop: &AtomicBool,
        output_sender: &crossbeam::channel::Sender<FrameResult>,
    ) -> usize {
        re_tracing::profile_function!();
        let mut count = 0;
        while !should_stop.load(Ordering::SeqCst) {
            let picture = {
                econtext::econtext!("get_picture");
                self.decoder.get_picture()
            };
            match picture {
                Ok(picture) => {
                    let frame = create_frame(&self.debug_name, &picture);
                    output_sender.send(frame).ok();
                    count += 1;
                }
                Err(dav1d::Error::Again) => {
                    // We need to submit more chunks to get more pictures
                    break;
                }
                Err(err) => {
                    output_sender.send(Err(DecodeError::Dav1d(err))).ok();
                }
            }
        }
        count
    }
}

fn create_frame(debug_name: &str, picture: &dav1d::Picture) -> FrameResult {
    re_tracing::profile_function!();

    let bits_per_component = picture
        .bits_per_component()
        .map_or_else(|| picture.bit_depth(), |bpc| bpc.0);

    let bytes_per_component = if bits_per_component == 8 {
        1
    } else if 8 < bits_per_component && bits_per_component <= 16 {
        // TODO(#7594): Support HDR video.
        // We currently handle HDR videos by throwing away the lowest bits,
        // and doing so rather slowly on CPU. It works, but the colors won't be perfectly correct.
        // Note that `bit_depth` is either 8 or 16, which is semi-independent `bits_per_component` (which is None/8/10/12).
        //
        // Don't trigger a warning here: we already warn separately for detecting 10bit elsewhere as well as for HDR primaries.
        2
    } else {
        return Err(DecodeError::BadBitsPerComponent(bits_per_component));
    };

    let mut data = match picture.pixel_layout() {
        // Monochrome
        PixelLayout::I400 => picture.plane(PlanarImageComponent::Y).to_vec(),

        PixelLayout::I420 | PixelLayout::I422 | PixelLayout::I444 => {
            let height_y = picture.height() as usize;
            let height_uv = match picture.pixel_layout() {
                PixelLayout::I400 => 0,
                PixelLayout::I420 => height_y / 2,
                PixelLayout::I422 | PixelLayout::I444 => height_y,
            };

            let packed_stride_y = bytes_per_component * picture.width() as usize;
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
    };

    if bytes_per_component == 2 {
        re_tracing::profile_scope!("Truncate HDR"); // costs around 1.5ms per megapixel on MacBook Pro M3 Max
        let rshift = bits_per_component - 8; // we throw away the low bits
        data = data
            .chunks(2)
            .map(|c| {
                let lo = c[0] as u16;
                let hi = c[1] as u16;
                let full = (hi << 8) | lo;
                (full >> rshift) as u8
            })
            .collect();
    }

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
        coefficients: yuv_matrix_coefficients(debug_name, picture),
    };

    Ok(Frame {
        content: FrameContent {
            data,
            width: picture.width(),
            height: picture.height(),
            format,
        },
        info: FrameInfo {
            // TODO(andreas): dav1d has a user-data field that isn't exposed yet.
            // We should us that to populate these fields.
            is_sync: None,
            sample_idx: None,
            frame_nr: None,
            latest_decode_timestamp: None,

            presentation_timestamp: Time(picture.timestamp().unwrap_or(0)),
            duration: (picture.duration() != 0).then_some(Time(picture.duration())),
        },
    })
}

fn yuv_matrix_coefficients(debug_name: &str, picture: &dav1d::Picture) -> YuvMatrixCoefficients {
    // Quotes are from https://wiki.x266.mov/docs/colorimetry/matrix (if not noted otherwise)
    match picture.matrix_coefficients() {
        dav1d::pixel::MatrixCoefficients::Identity => YuvMatrixCoefficients::Identity,

        dav1d::pixel::MatrixCoefficients::BT709 => YuvMatrixCoefficients::Bt709,

        dav1d::pixel::MatrixCoefficients::Unspecified
        | dav1d::pixel::MatrixCoefficients::Reserved => {
            // This happens quite often. Don't issue a warning, that would be noise!

            #[expect(clippy::branches_sharing_code)]
            if picture.transfer_characteristic() == dav1d::pixel::TransferCharacteristic::SRGB {
                // If the transfer characteristic is sRGB, assume BT.709 primaries, would be quite odd otherwise.
                // TODO(andreas): Other transfer characteristics may also hint at primaries.
                YuvMatrixCoefficients::Bt709
            } else {
                // Best guess: If the picture is 720p+ assume Bt709 because Rec709
                // is the "HDR" standard.
                // TODO(#7594): 4k/UHD material should probably assume Bt2020?
                // else if picture.height() >= 720 {
                //     YuvMatrixCoefficients::Bt709
                // } else {
                //     YuvMatrixCoefficients::Bt601
                // }
                //
                // This is also what the mpv player does (and probably others):
                // https://wiki.x266.mov/docs/colorimetry/matrix#2-unspecified
                // (and similar for primaries! https://wiki.x266.mov/docs/colorimetry/primaries#2-unspecified)
                //
                // …then again, eyeballing VLC it looks like it just always assumes BT.709.
                // The handwavy test case employed here was the same video in low & high resolution
                // without specified primaries. Both looked the same.
                YuvMatrixCoefficients::Bt709
            }
        }

        dav1d::pixel::MatrixCoefficients::BT470M => {
            // "BT.470M is a standard that was used in analog television systems in the United States."
            // I guess Bt601 will do!
            YuvMatrixCoefficients::Bt601
        }
        dav1d::pixel::MatrixCoefficients::BT470BG | dav1d::pixel::MatrixCoefficients::ST170M => {
            // This is PAL & NTSC standards, both are part of Bt.601.
            YuvMatrixCoefficients::Bt601
        }
        dav1d::pixel::MatrixCoefficients::ST240M => {
            // "SMPTE 240M was an interim standard used during the early days of HDTV (1988-1998)."
            // Not worth the effort: HD -> Bt709 🤷
            YuvMatrixCoefficients::Bt709
        }

        dav1d::pixel::MatrixCoefficients::BT2020NonConstantLuminance
        | dav1d::pixel::MatrixCoefficients::BT2020ConstantLuminance
        | dav1d::pixel::MatrixCoefficients::ICtCp
        | dav1d::pixel::MatrixCoefficients::ST2085 => {
            // TODO(#7594): HDR support (we'll probably only care about `BT2020NonConstantLuminance`?)
            re_log::warn_once!(
                "Video {debug_name:?} specified HDR color primaries. Rerun doesn't handle HDR colors correctly yet. Color artifacts may be visible."
            );
            YuvMatrixCoefficients::Bt709
        }

        dav1d::pixel::MatrixCoefficients::ChromaticityDerivedNonConstantLuminance
        | dav1d::pixel::MatrixCoefficients::ChromaticityDerivedConstantLuminance
        | dav1d::pixel::MatrixCoefficients::YCgCo => {
            re_log::warn_once!(
                "Video {debug_name:?} specified unsupported matrix coefficients {:?}. Color artifacts may be visible.",
                picture.matrix_coefficients()
            );
            YuvMatrixCoefficients::Bt709
        }
    }
}
