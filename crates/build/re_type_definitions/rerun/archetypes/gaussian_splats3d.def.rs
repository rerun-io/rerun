// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D gaussian splats, e.g. from a 3D Gaussian Splatting (3DGS) reconstruction.
///
/// Each gaussian is an anisotropic 3D gaussian distribution:
/// a unit isotropic gaussian scaled per-axis, rotated, and translated.
/// The scales are the standard deviations along the principal axes, in scene units.
///
/// Note that unlike 3DGS training checkpoints (e.g. PLY files), all values are stored
/// in their natural form: linear scales, and base colors with opacity as RGBA.
///
/// \example archetypes/gaussian_splats3d_simple title="Log a few gaussian splats"
/// \example archetypes/gaussian_splats3d_ply title="Log a 3D gaussian splatting (3DGS) PLY file"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(unreleased)]
#[docs(view_types = "Spatial3DView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "GaussianSplats3D")]
#[rust(derive(PartialEq))]
pub struct GaussianSplats3D {
    /// The centers (means) of the gaussians.
    #[rerun(required)]
    pub centers: Vec<rerun::components::Position3D>,

    /// Per-axis standard deviations of the gaussians, in scene units.
    #[rerun(recommended)]
    pub scales: Option<Vec<rerun::components::Scale3D>>,

    /// The orientations of the gaussians.
    #[rerun(recommended)]
    pub quaternions: Option<Vec<rerun::components::RotationQuat>>,

    /// The base colors and opacities of the gaussians.
    ///
    /// The RGB part is the view-independent base color, i.e. the degree-0 (DC) term of the spherical harmonics.
    /// The alpha part is the peak opacity of the gaussian; the gaussian falloff further modulates it spatially.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Higher-order spherical harmonics coefficients for view-dependent color.
    #[rerun(optional)]
    pub sh_coefficients: Option<Vec<rerun::components::SphericalHarmonics3Rgb>>,

    /// The highest spherical harmonics degree to evaluate when rendering, 0-3.
    ///
    /// Lower values render faster; `0` disables view-dependent color entirely.
    /// If not set, defaults to 3, i.e. all coefficients present in the data are used.
    #[rerun(optional)]
    pub spherical_harmonics_degree: Option<rerun::components::SphericalHarmonicsDegree>,
}
