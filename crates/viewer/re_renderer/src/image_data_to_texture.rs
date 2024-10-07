use crate::{resource_managers::GpuTexture2D, DebugLabel, RenderContext};

/// Type of color space a given image is in.
///
/// This applies both to YCbCr and RGB formats, but if not specified otherwise
/// we assume BT.709 primaries for all RGB(A) 8bits per channel content (details below on [`ColorSpace::Bt709`]).
/// Since with YCbCr content the color space is often less clear, we always explicitely
/// specify it.
///
/// Ffmpeg's documentation has a short & good overview of these relationships:
/// https://trac.ffmpeg.org/wiki/colorspace#WhatiscolorspaceWhyshouldwecare
pub enum ColorSpace {
    /// BT.601 (aka. SDTV, aka. Rec.601)
    ///
    /// Wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
    Bt601,

    /// BT.709 (aka. HDTV, aka. Rec.709)
    ///
    /// Wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
    ///
    /// These are the same primaries we usually assume and use for all our rendering
    /// since they are the same primaries used by sRGB.
    /// https://en.wikipedia.org/wiki/Rec._709#Relationship_to_sRGB
    /// The OETF/EOTF function (https://en.wikipedia.org/wiki/Transfer_functions_in_imaging) is different,
    /// but for all other purposes they are the same.
    /// (The only reason for us to convert to optical units ("linear" instead of "gamma") is for
    /// lighting & tonemapping where we typically start out with an sRGB image!)
    Bt709,
    //
    // Not yet supported. These vary a lot more from the other two!
    //
    // /// BT.2020 (aka. PQ, aka. Rec.2020)
    // ///
    // /// Wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.2020_conversion
    // BT2020_ConstantLuminance,
    // BT2020_NonConstantLuminance,
}

/// Image data format that can be converted to a wgpu texture.
pub enum SourceImageDataFormat {
    /// 8-bit per channel RGBA, with alpha.
    ///
    /// This is a no-op for the converter and will just directly upload the data.
    SrgbRgba8,

    /// 8-bit per channel RGB, no alpha.
    // SrgbRgb8,

    // /// 8-bit per channel BGRA, with alpha.
    // ///
    // /// This is a no-op for the converter and will just directly upload the data.
    // SrgbBgra8,

    // /// 8-bit per channel BGR, no alpha.
    // SrgbBgr8,

    // -----------------------------------------------------------
    // Chroma downsampled formats, using YCbCr format.
    // -----------------------------------------------------------
    /// NV12` (aka `Y_UV12`) is a YUV 4:2:0 chroma downsampled format with 12 bits per pixel and 8 bits per channel.
    ///
    /// First comes entire image in Y in one plane,
    /// followed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc.
    Nv12(ColorSpace),

    /// `YUY2` (aka `YUYV` or `YUYV16`), is a YUV 4:2:2 chroma downsampled format with 16 bits per pixel and 8 bits per channel.
    ///
    /// The order of the channels is Y0, U0, Y1, V0, all in the same plane.
    Yuy2(ColorSpace),
}

/// Error that can occur when converting image data to a texture.
#[derive(thiserror::Error, Debug)]
enum ImageDataToTextureError {
    #[error("Invalid data length. Expected {expected} bytes, got {actual} bytes")]
    InvalidDataLength { expected: usize, actual: usize },
    // TODO: more.
}

/// Takes raw image data and transfers & converts it to a GPU texture.
///
/// Schedules render passes to convert the data to a samplable textures as needed.
///
/// Implementation note:
/// Since we're targeting WebGL, all data has always to be uploaded into textures (we can't use raw buffers!).
/// Buffer->Texture copies have restrictions on row padding, so any approach where we first
/// allocate gpu readable memory and hand it to the user would make the API a lot more complicated.
pub fn transfer_image_data_to_texture(
    render_ctx: &RenderContext,
    texture_label: DebugLabel,
    width: u32,
    height: u32,
    source_format: SourceImageDataFormat,
    data: &[u8],
) -> Result<GpuTexture2D, ImageDataToTextureError> {
    // TODO: validate that combination of data, format and size makes sense.

    // TODO: allocate gpu belt data and upload it
    // Take care of row padding etc.!
    // Depending on source format this may already do the entire work (like inserting bytes or rearranging things)

    // TODO: if needed, schedule render pass with fragment shader to convert data
    // -> this may need render pipelines & bind layouts.
    //    -> Just use `ctx.renderer` in order to encapsulate the necessary data for different conversion steps.
    //       Bit of a missuse but should work & scale (!) just fine.

    Ok(())
}
