//! Tests mainly, but not exclusively, of [`re_types::archetypes`].

// Test helpers

mod util;

// Tests of archetypes and their related components and datatypes

mod annotation_context;
mod arrows3d;
mod asset3d;
mod box2d;
mod box3d;
mod clear;
mod depth_image;
mod line_strips2d;
mod line_strips3d;
mod mesh3d;
mod pinhole;
mod points2d;
mod points3d;
mod segmentation_image;
mod tensor;
mod text_document;
mod transform3d;
mod view_coordinates;

// Tests of other things

#[cfg(feature = "testing")]
mod fuzzy;
#[cfg(feature = "mint")]
mod mint_conversions;
mod validity;
