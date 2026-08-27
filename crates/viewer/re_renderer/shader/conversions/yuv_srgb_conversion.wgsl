#import <../types.wgsl>

// see `enum YuvMatrixCoefficients`.
const COEFFS_IDENTITY = 0u;
const COEFFS_BT601 = 1u;
const COEFFS_BT709 = 2u;

// see `enum YuvRange`.
const YUV_RANGE_LIMITED = 0u;
const YUV_RANGE_FULL = 1u;

/// Returns sRGB from YUV color.
///
/// Expects the YUV components in the 0-255 range.
///
/// This conversion mirrors the function in `crates/store/re_sdk_types/src/encodings/tensor_data_ext.rs`
///
/// Specifying the color standard should be exposed in the future [#3541](https://github.com/rerun-io/rerun/pull/3541)
fn srgb_from_yuv(yuv: vec3f, yuv_matrix_coefficients: u32, range: u32) -> vec3f {
    // rescale YUV values
    //
    // This is what is called "limited range" and is the most common case.
    // TODO(andreas): Support "full range" as well.

    var y: f32;
    var u: f32;
    var v: f32;

    switch (range) {
        case YUV_RANGE_LIMITED: {
            // Via https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion:
            // "The resultant signals range from 16 to 235 for Y′ (Cb and Cr range from 16 to 240);
            // the values from 0 to 15 are called footroom, while the values from 236 to 255 are called headroom."
            y = (yuv[0] - 16.0) / 219.0;
            u = (yuv[1] - 128.0) / 224.0;
            v = (yuv[2] - 128.0) / 224.0;
        }

        case YUV_RANGE_FULL: {
            y = yuv[0] / 255.0;
            u = (yuv[1] - 128.0) / 255.0;
            v = (yuv[2] - 128.0) / 255.0;
        }

        default: {
            // Should never happen.
            return ERROR_RGBA.rgb;
        }
    }

    var rgb: vec3f;

    switch (yuv_matrix_coefficients) {
        case COEFFS_IDENTITY: {
            // u & v have a range from -0.5 to 0.5. Bring them back to 0-1.
            rgb = vec3f(v + 0.5, y, u + 0.5);
        }

        // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
        // Also note according to https://en.wikipedia.org/wiki/SRGB#sYCC_extended-gamut_transformation
        // > Although the RGB color primaries are based on BT.709,
        // > the equations for transformation from sRGB to sYCC and vice versa are based on BT.601.
        case COEFFS_BT601: {
            rgb.r = y + 1.402 * v;
            rgb.g = y - 0.344 * u - 0.714 * v;
            rgb.b = y + 1.772 * u;
        }

        // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
        case COEFFS_BT709: {
            rgb.r = y + 1.575 * v;
            rgb.g = y - 0.187 * u - 0.468 * v;
            rgb.b = y + 1.856 * u;
        }

        default: {
            return ERROR_RGBA.rgb;
        }
    }

    return clamp(rgb, vec3f(0.0), vec3f(1.0));
}
