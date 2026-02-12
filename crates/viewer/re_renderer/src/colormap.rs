#![expect(clippy::excessive_precision)]

use glam::{Vec2, Vec3A, Vec4, Vec4Swizzles as _};
use re_log::debug_assert;

// ---

// NOTE: Keep in sync with `colormap.wgsl`!
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u32)]
pub enum Colormap {
    // Reserve 0 for "disabled"
    /// sRGB gray gradient = perceptually even
    #[default]
    Grayscale = 1,
    Inferno = 2,
    Magma = 3,
    Plasma = 4,
    Turbo = 5,
    Viridis = 6,
    CyanToYellow = 7,
    Spectral = 8,
    Twilight = 9,
}

impl Colormap {
    pub const ALL: [Self; 9] = [
        Self::Grayscale,
        Self::Inferno,
        Self::Magma,
        Self::Plasma,
        Self::Turbo,
        Self::Viridis,
        Self::CyanToYellow,
        Self::Spectral,
        Self::Twilight,
    ];
}

impl std::fmt::Display for Colormap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Grayscale => write!(f, "Grayscale"),
            Self::Inferno => write!(f, "Inferno"),
            Self::Magma => write!(f, "Magma"),
            Self::Plasma => write!(f, "Plasma"),
            Self::Turbo => write!(f, "Turbo"),
            Self::Viridis => write!(f, "Viridis"),
            Self::CyanToYellow => write!(f, "CyanToYellow"),
            Self::Spectral => write!(f, "Spectral"),
            Self::Twilight => write!(f, "Twilight"),
        }
    }
}

pub fn colormap_srgb(which: Colormap, t: f32) -> [u8; 4] {
    match which {
        Colormap::Grayscale => grayscale_srgb(t),
        Colormap::Turbo => colormap_turbo_srgb(t),
        Colormap::Viridis => colormap_viridis_srgb(t),
        Colormap::Plasma => colormap_plasma_srgb(t),
        Colormap::Magma => colormap_magma_srgb(t),
        Colormap::Inferno => colormap_inferno_srgb(t),
        Colormap::CyanToYellow => colormap_cyan_to_yellow_srgb(t),
        Colormap::Spectral => colormap_spectral_srgb(t),
        Colormap::Twilight => colormap_twilight_srgb(t),
    }
}

/// Returns an sRGB gray value, assuming `t` is normalized.
pub fn grayscale_srgb(t: f32) -> [u8; 4] {
    debug_assert!((0.0..=1.0).contains(&t));

    let t = ((t * u8::MAX as f32) + 0.5) as u8;

    [t, t, t, 255]
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
// Data fitted from https://github.com/BIDS/colormap/blob/bc549477db0c12b54a5928087552ad2cf274980f/colormaps.py (CC0).

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

// --- rasmusgo's color maps ---

// Designed by Rasmus Brönnegård (rasmusgo) adapted from https://www.shadertoy.com/view/lfByRh.
//
// License CC0 (public domain)
//   https://creativecommons.org/share-your-work/public-domain/cc0/

/// Returns a gamma-space sRGB in 0-255 range.
///
/// This is a perceptually uniform colormap which is robust to color blindness.
/// It is especially suited for visualizing signed values.
/// It interpolates from cyan to blue to dark gray to brass to yellow.
pub fn colormap_cyan_to_yellow_srgb(t: f32) -> [u8; 4] {
    let t = t * 2. - 1.;
    [
        ((1. + 3. * t) * (255. / 4.)).max(0.) as u8,
        ((1. + 3. * t * t) * (255. / 4.)) as u8,
        ((1. - 3. * t) * (255. / 4.)).max(0.) as u8,
        255,
    ]
}

/// Returns sRGB polynomial approximation from Spectral color map, assuming `t` is normalized.
pub fn colormap_spectral_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(0.584384543712538, 0.006424432561482, 0.231061410304836);
    const C1: Vec3A = Vec3A::new(3.768572852617221, 2.487082885717158, 2.821174312084977);
    const C2: Vec3A = Vec3A::new(-16.262574054760623, -6.243215992229093, -37.292187460541960);
    const C3: Vec3A = Vec3A::new(39.821464952234010, 39.932449794574126, 186.340471613899751);
    const C4: Vec3A = Vec3A::new(
        -46.140976850412727,
        -99.423798167148249,
        -388.532539629914481,
    );
    const C5: Vec3A = Vec3A::new(12.716626092708825, 96.954180217298671, 360.627239851094203);
    const C6: Vec3A = Vec3A::new(5.942343111972585, -33.440386037285862, -123.635206049211334);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));

    let c = c * 255.0;
    [c.x as u8, c.y as u8, c.z as u8, 255]
}

/// Returns sRGB polynomial approximation from Twilight color map, assuming `t` is normalized.
///
/// This is a perceptually uniform cyclic colormap from Matplotlib, it is useful for
/// visualizing periodic or cyclic data.
///
/// It interpolates from white to blue to purple to red to orange and back to white.
///
/// Data from <https://github.com/matplotlib/matplotlib> (matplotlib's twilight colormap).
pub fn colormap_twilight_srgb(t: f32) -> [u8; 4] {
    const C0: Vec3A = Vec3A::new(0.99435322698120, 0.85170793387210, 0.93942033498486);
    const C1: Vec3A = Vec3A::new(-6.61774273956635, -0.23133259259568, -3.96704343424284);
    const C2: Vec3A = Vec3A::new(41.78124131041812, -7.61851602599826, 38.98566990464263);
    const C3: Vec3A = Vec3A::new(-158.29764239605322, 3.73408709288658, -170.02538195370874);
    const C4: Vec3A = Vec3A::new(301.70954078396789, 25.04157831823896, 319.73628266524258);
    const C5: Vec3A = Vec3A::new(-265.16454480601146, -30.83148395246298, -271.62226902484138);

    // Adjusted C6 to ensure f(0) = f(1) for true cyclicity
    const C6: Vec3A = Vec3A::new(86.58914784721200, 9.90660484718943, 86.89294583380010);

    debug_assert!((0.0..=1.0).contains(&t));

    let c = C0 + t * (C1 + t * (C2 + t * (C3 + t * (C4 + t * (C5 + t * C6)))));
    let c = c * 255.0;

    [c.x as u8, c.y as u8, c.z as u8, 255]
}
