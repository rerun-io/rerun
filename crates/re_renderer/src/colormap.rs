#![allow(clippy::excessive_precision)]

use glam::{Vec2, Vec3A, Vec4, Vec4Swizzles};

// ---

// NOTE: Keep in sync with `colormap.wgsl`!
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ColorMap {
    Grayscale = 0,
    ColorMapTurbo = 1,
    ColorMapViridis = 2,
    ColorMapPlasma = 3,
    ColorMapMagma = 4,
    ColorMapInferno = 5,
}

pub fn colormap_srgb(which: ColorMap, t: f32) -> [u8; 4] {
    match which {
        ColorMap::Grayscale => grayscale_srgb(t),
        ColorMap::ColorMapTurbo => colormap_turbo_srgb(t),
        ColorMap::ColorMapViridis => colormap_viridis_srgb(t),
        ColorMap::ColorMapPlasma => colormap_plasma_srgb(t),
        ColorMap::ColorMapMagma => colormap_magma_srgb(t),
        ColorMap::ColorMapInferno => colormap_inferno_srgb(t),
    }
}

/// Returns an sRGB gray value, assuming `t` is normalized.
pub fn grayscale_srgb(t: f32) -> [u8; 4] {
    debug_assert!((0.0..=1.0).contains(&t));

    let t = t.powf(2.2);
    let t = ((t * u8::MAX as f32) + 0.5) as u8;

    [t, t, t, t]
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

/// Returns sRGB polynomial approximation from Turbo color map, assuming `t` is normalized.
pub fn colormap_turbo_srgb(t: f32) -> [u8; 4] {
    const R4: Vec4 = Vec4::new(0.13572138, 4.61539260, -42.66032258, 132.13108234);
    const G4: Vec4 = Vec4::new(0.09140261, 2.19418839, 4.84296658, -14.18503333);
    const B4: Vec4 = Vec4::new(0.10667330, 12.64194608, -60.58204836, 110.36276771);

    const R2: Vec2 = Vec2::new(-152.94239396, 59.28637943);
    const G2: Vec2 = Vec2::new(4.27729857, 2.82956604);
    const B2: Vec2 = Vec2::new(-89.90310912, 27.34824973);

    debug_assert!((0.0..=1.0).contains(&t));

    let v4 = glam::vec4(1.0, t, t * t, t * t * t);
    let v2 = v4.zw() * v4.z;

    [
        ((v4.dot(R4) + v2.dot(R2)) * 255.0) as u8,
        ((v4.dot(G4) + v2.dot(G2)) * 255.0) as u8,
        ((v4.dot(B4) + v2.dot(B2)) * 255.0) as u8,
        255,
    ]
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

/// Returns sRGB polynomial approximation from Viridis color map, assuming `t` is normalized.
pub fn colormap_viridis_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(0.2777273272234177, 0.005407344544966578, 0.3340998053353061);
    const C1: Vec3A = Vec3A::new(0.1050930431085774, 1.404613529898575, 1.384590162594685);
    const C2: Vec3A = Vec3A::new(-0.3308618287255563, 0.214847559468213, 0.09509516302823659);
    const C3: Vec3A = Vec3A::new(-4.634230498983486, -5.799100973351585, -19.33244095627987);
    const C4: Vec3A = Vec3A::new(6.228269936347081, 14.17993336680509, 56.69055260068105);
    const C5: Vec3A = Vec3A::new(4.776384997670288, -13.74514537774601, -65.35303263337234);
    const C6: Vec3A = Vec3A::new(-5.435455855934631, 4.645852612178535, 26.3124352495832);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));

    let c = c * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}

/// Returns sRGB polynomial approximation from Plasma color map, assuming `t` is normalized.
pub fn colormap_plasma_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(0.05873234392399702, 0.02333670892565664, 0.5433401826748754);
    const C1: Vec3A = Vec3A::new(2.176514634195958, 0.2383834171260182, 0.7539604599784036);
    const C2: Vec3A = Vec3A::new(-2.689460476458034, -7.455851135738909, 3.110799939717086);
    const C3: Vec3A = Vec3A::new(6.130348345893603, 42.3461881477227, -28.51885465332158);
    const C4: Vec3A = Vec3A::new(-11.10743619062271, -82.66631109428045, 60.13984767418263);
    const C5: Vec3A = Vec3A::new(10.02306557647065, 71.41361770095349, -54.07218655560067);
    const C6: Vec3A = Vec3A::new(-3.658713842777788, -22.93153465461149, 18.19190778539828);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));

    let c = c * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}

/// Returns sRGB polynomial approximation from Magma color map, assuming `t` is normalized.
pub fn colormap_magma_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(-0.002136485053939, -0.000749655052795, -0.005386127855323);
    const C1: Vec3A = Vec3A::new(0.2516605407371642, 0.6775232436837668, 2.494026599312351);
    const C2: Vec3A = Vec3A::new(8.353717279216625, -3.577719514958484, 0.3144679030132573);
    const C3: Vec3A = Vec3A::new(-27.66873308576866, 14.26473078096533, -13.64921318813922);
    const C4: Vec3A = Vec3A::new(52.17613981234068, -27.94360607168351, 12.94416944238394);
    const C5: Vec3A = Vec3A::new(-50.76852536473588, 29.04658282127291, 4.23415299384598);
    const C6: Vec3A = Vec3A::new(18.65570506591883, -11.48977351997711, -5.601961508734096);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));

    let c = c * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}

/// Returns sRGB polynomial approximation from Inferno color map, assuming `t` is normalized.
pub fn colormap_inferno_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(0.00021894036911922, 0.0016510046310010, -0.019480898437091);
    const C1: Vec3A = Vec3A::new(0.1065134194856116, 0.5639564367884091, 3.932712388889277);
    const C2: Vec3A = Vec3A::new(11.60249308247187, -3.972853965665698, -15.9423941062914);
    const C3: Vec3A = Vec3A::new(-41.70399613139459, 17.43639888205313, 44.35414519872813);
    const C4: Vec3A = Vec3A::new(77.162935699427, -33.40235894210092, -81.80730925738993);
    const C5: Vec3A = Vec3A::new(-71.31942824499214, 32.62606426397723, 73.20951985803202);
    const C6: Vec3A = Vec3A::new(25.13112622477341, -12.24266895238567, -23.07032500287172);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));

    let c = c * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}
