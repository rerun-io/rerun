//! A miniature version of the definitions crate.
//!
//! The test is that this file compiles: it exercises every annotation `#[rerun_type]` has to
//! strip, the `extern crate self as rerun;` trick that makes `rerun::`-rooted paths resolve with
//! no `use` statements anywhere, and the two prelude types that have no spelling in plain Rust.
//!
//! Several things in here would be hard errors without `#[rerun_type]`: `#[rerun(…)]` and friends
//! are registered nowhere, and `#[default]` normally requires a companion `#[derive(Default)]`.
//! `#[repr(…)]` is deliberately *not* stripped, so rustc still checks it.

extern crate self as rerun;

pub use re_types_builder_prelude::{Binary, f16, rerun_type};

pub mod datatypes {
    /// A vector in 3D space.
    #[rerun::rerun_type]
    #[rerun(state = "stable")]
    #[rust(derive(Default, Copy, bytemuck::Pod, bytemuck::Zeroable))]
    #[arrow(transparent)]
    #[repr(transparent)]
    pub struct Vec3D {
        pub xyz: [f32; 3],
    }

    /// The types the frontend has to be able to spell, including the prelude's own.
    #[rerun::rerun_type]
    #[rerun(state = "unstable")]
    #[rust(derive_only(Clone, PartialEq))]
    pub enum TensorBuffer {
        /// 8bit unsigned integer.
        U8(Vec<u8>),

        /// 16bit IEEE-754 floating point, also known as `half`.
        F16(Vec<rerun::f16>),

        /// A list of bytes of arbitrary length.
        Bytes(rerun::Binary),
    }

    /// A C-style enum, with an explicit wire value per variant.
    #[rerun::rerun_type]
    #[rerun(state = "stable")]
    #[repr(u8)]
    pub enum ColorModel {
        /// Red, green, blue.
        #[default]
        Rgb = 1,

        /// Red, green, blue, alpha.
        Rgba = 2,
    }
}

pub mod components {
    /// A position in 3D space.
    #[rerun::rerun_type]
    #[rerun(state = "stable")]
    #[python(aliases = "npt.NDArray[Any] | Sequence[float]")]
    pub struct Position3D(pub rerun::datatypes::Vec3D);
}

pub mod archetypes {
    /// A 3D point cloud with positions and optional colors, radii, labels, etc.
    #[rerun::rerun_type]
    #[rerun(state = "stable")]
    #[docs(category = "Spatial 3D", view_types = "Spatial3DView")]
    pub struct Points3D {
        /// All the 3D positions at which the point cloud shows points.
        #[rerun(required)]
        pub positions: Vec<rerun::components::Position3D>,

        /// Which color model the point cloud is in, if any.
        #[rerun(optional)]
        #[cpp(rename_field = "color_model_")]
        pub color_model: Option<rerun::datatypes::ColorModel>,
    }
}

/// A definition file is never linked into anything, so there is nothing to assert at runtime —
/// compiling this file *is* the test.
#[test]
fn definitions_compile() {}
