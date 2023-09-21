#import <./types.wgsl>
#import <./utils/srgb.wgsl>

// NOTE: Keep in sync with `colormap.rs`!
const COLORMAP_GRAYSCALE: u32 = 1u; // sRGB gradient = perceptually even
const COLORMAP_INFERNO:   u32 = 2u;
const COLORMAP_MAGMA:     u32 = 3u;
const COLORMAP_PLASMA:    u32 = 4u;
const COLORMAP_TURBO:     u32 = 5u;
const COLORMAP_VIRIDIS:   u32 = 6u;

/// Returns a gamma-space sRGB in 0-1 range.
///
/// The input will be saturated to [0, 1] range.
fn colormap_srgb(which: u32, t_unsaturated: f32) -> Vec3 {
    let t = saturate(t_unsaturated);
    if which == COLORMAP_GRAYSCALE {
        // A linear gray gradient in sRGB gamma space is supposed to be perceptually even as-is!
        // Easy to get confused: A linear gradient in sRGB linear space is *not* perceptually even.
        return Vec3(t);
    } else if which == COLORMAP_INFERNO {
        return colormap_inferno_srgb(t);
    } else if which == COLORMAP_MAGMA {
        return colormap_magma_srgb(t);
    } else if which == COLORMAP_PLASMA {
        return colormap_plasma_srgb(t);
    } else if which == COLORMAP_TURBO {
        return colormap_turbo_srgb(t);
    } else if which == COLORMAP_VIRIDIS {
        return colormap_viridis_srgb(t);
    } else {
        return ERROR_RGBA.rgb;
    }
}

/// Returns a linear-space sRGB in 0-1 range.
///
/// The input will be saturated to [0, 1] range.
fn colormap_linear(which: u32, t: f32) -> Vec3 {
    return linear_from_srgb(colormap_srgb(which, t));
}

// --- Turbo color map ---

// Polynomial approximation in GLSL for the Turbo colormap.
// Taken from https://gist.github.com/mikhailov-work/0d177465a8151eb6ede1768d51d476c7.
// Original LUT: https://gist.github.com/mikhailov-work/ee72ba4191942acecc03fe6da94fc73f.
//
// Copyright 2019 Google LLC.
// SPDX-License-Identifier: Apache-2.0
//
// Authors:
//   Colormap Design: Anton Mikhailov (mikhailov@google.com)
//   GLSL Approximation: Ruofei Du (ruofei@google.com)

/// Returns a gamma-space sRGB in 0-1 range.
/// This is a polynomial approximation from Turbo color map, assuming `t` is
/// normalized (it will be saturated no matter what).
fn colormap_turbo_srgb(t: f32) -> Vec3 {
    let r4 = Vec4(0.13572138, 4.61539260, -42.66032258, 132.13108234);
    let g4 = Vec4(0.09140261, 2.19418839, 4.84296658, -14.18503333);
    let b4 = Vec4(0.10667330, 12.64194608, -60.58204836, 110.36276771);
    let r2 = Vec2(-152.94239396, 59.28637943);
    let g2 = Vec2(4.27729857, 2.82956604);
    let b2 = Vec2(-89.90310912, 27.34824973);

    let v4 = vec4(1.0, t, t * t, t * t * t);
    let v2 = v4.zw * v4.z;

    return vec3(
        dot(v4, r4) + dot(v2, r2),
        dot(v4, g4) + dot(v2, g2),
        dot(v4, b4) + dot(v2, b2)
    );
}

// --- Matplotlib color maps ---

// Polynomials fitted to matplotlib colormaps, taken from https://www.shadertoy.com/view/WlfXRN.
//
// License CC0 (public domain)
//   https://creativecommons.org/share-your-work/public-domain/cc0/
//
// Similar to https://www.shadertoy.com/view/XtGGzG but with a couple small differences:
//  - use degree 6 instead of degree 5 polynomials
//  - use nested horner representation for polynomials
//  - polynomials were fitted to minimize maximum error (as opposed to least squares)
//
// Data fitted from https://github.com/BIDS/colormap/blob/master/colormaps.py (CC0).

/// Returns a gamma-space sRGB in 0-1 range.
/// This is a polynomial approximation from Viridis color map, assuming `t` is
/// normalized (it will be saturated no matter what).
fn colormap_viridis_srgb(t: f32) -> Vec3 {
    let c0 = Vec3(0.2777273272234177, 0.005407344544966578, 0.3340998053353061);
    let c1 = Vec3(0.1050930431085774, 1.404613529898575, 1.384590162594685);
    let c2 = Vec3(-0.3308618287255563, 0.214847559468213, 0.09509516302823659);
    let c3 = Vec3(-4.634230498983486, -5.799100973351585, -19.33244095627987);
    let c4 = Vec3(6.228269936347081, 14.17993336680509, 56.69055260068105);
    let c5 = Vec3(4.776384997670288, -13.74514537774601, -65.35303263337234);
    let c6 = Vec3(-5.435455855934631, 4.645852612178535, 26.3124352495832);
    return c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
}

/// Returns a gamma-space sRGB in 0-1 range.
/// This is a polynomial approximation from Plasma color map, assuming `t` is
/// normalized (it will be saturated no matter what).
fn colormap_plasma_srgb(t: f32) -> Vec3 {
    let c0 = Vec3(0.05873234392399702, 0.02333670892565664, 0.5433401826748754);
    let c1 = Vec3(2.176514634195958, 0.2383834171260182, 0.7539604599784036);
    let c2 = Vec3(-2.689460476458034, -7.455851135738909, 3.110799939717086);
    let c3 = Vec3(6.130348345893603, 42.3461881477227, -28.51885465332158);
    let c4 = Vec3(-11.10743619062271, -82.66631109428045, 60.13984767418263);
    let c5 = Vec3(10.02306557647065, 71.41361770095349, -54.07218655560067);
    let c6 = Vec3(-3.658713842777788, -22.93153465461149, 18.19190778539828);
    return c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
}

/// Returns a gamma-space sRGB in 0-1 range.
/// This is a polynomial approximation from Magma color map, assuming `t` is
/// normalized (it will be saturated no matter what).
fn colormap_magma_srgb(t: f32) -> Vec3 {
    let c0 = Vec3(-0.002136485053939582, -0.000749655052795221, -0.005386127855323933);
    let c1 = Vec3(0.2516605407371642, 0.6775232436837668, 2.494026599312351);
    let c2 = Vec3(8.353717279216625, -3.577719514958484, 0.3144679030132573);
    let c3 = Vec3(-27.66873308576866, 14.26473078096533, -13.64921318813922);
    let c4 = Vec3(52.17613981234068, -27.94360607168351, 12.94416944238394);
    let c5 = Vec3(-50.76852536473588, 29.04658282127291, 4.23415299384598);
    let c6 = Vec3(18.65570506591883, -11.48977351997711, -5.601961508734096);
    return c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
}

/// Returns a gamma-space sRGB in 0-1 range.
/// This is a polynomial approximation from Inferno color map, assuming `t` is
/// normalized (it will be saturated no matter what).
fn colormap_inferno_srgb(t: f32) -> Vec3 {
    let c0 = Vec3(0.0002189403691192265, 0.001651004631001012, -0.01948089843709184);
    let c1 = Vec3(0.1065134194856116, 0.5639564367884091, 3.932712388889277);
    let c2 = Vec3(11.60249308247187, -3.972853965665698, -15.9423941062914);
    let c3 = Vec3(-41.70399613139459, 17.43639888205313, 44.35414519872813);
    let c4 = Vec3(77.162935699427, -33.40235894210092, -81.80730925738993);
    let c5 = Vec3(-71.31942824499214, 32.62606426397723, 73.20951985803202);
    let c6 = Vec3(25.13112622477341, -12.24266895238567, -23.07032500287172);
    return c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
}
