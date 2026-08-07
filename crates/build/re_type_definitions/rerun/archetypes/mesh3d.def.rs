// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 3D triangle mesh as specified by its per-mesh and per-vertex properties.
///
/// See also [archetypes.Asset3D].
///
/// If there are multiple [archetypes.InstancePoses3D] instances logged to the same entity as a mesh,
/// an instance of the mesh will be drawn for each transform.
///
/// For transparency ordering, as well as back face culling (disabled by default),
/// front faces are assumed to be those with counter clockwise triangle winding order
/// (this is the same as in the GLTF specification).
///
/// \example archetypes/mesh3d_indexed title="Simple indexed 3D mesh" image="https://static.rerun.io/mesh3d_indexed/57c70dc992e6dc0bd9c5222ca084f5b6240cea75/1200w.png"
/// \example archetypes/mesh3d_instancing title="3D mesh with instancing" image="https://static.rerun.io/mesh3d_leaf_transforms3d/c2d0ee033129da53168f5705625a9b033f3a3d61/1200w.png"
/// \example archetypes/mesh3d_partial_updates !api title="Update specific parts of a 3D mesh over time" image="https://static.rerun.io/mesh3d_partial_updates/79b8a83294ef2c1eb7f9ae7dea7267a17da464ae/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Mesh3D")]
#[rust(derive(PartialEq))]
pub struct Mesh3D {
    /// The positions of each vertex.
    ///
    /// If no `triangle_indices` are specified, then each triplet of positions is interpreted as a triangle.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub vertex_positions: Vec<rerun::components::Position3D>,

    /// Optional indices for the triangles that make up the mesh.
    #[rerun(recommended)]
    pub triangle_indices: Option<Vec<rerun::components::TriangleIndices>>,

    /// An optional normal for each vertex.
    #[rerun(recommended)]
    pub vertex_normals: Option<Vec<rerun::components::Vector3D>>,

    /// An optional color for each vertex.
    ///
    /// The alpha channel is ignored.
    #[rerun(optional)]
    pub vertex_colors: Option<Vec<rerun::components::Color>>,

    /// An optional uv texture coordinate for each vertex.
    #[rerun(optional)]
    pub vertex_texcoords: Option<Vec<rerun::components::Texcoord2D>>,

    /// A color multiplier applied to the whole mesh.
    ///
    /// Alpha channel governs the overall mesh transparency.
    #[rerun(optional)]
    pub albedo_factor: Option<rerun::components::AlbedoFactor>,

    /// Determines which faces of the mesh are rendered.
    ///
    /// The default is [components.MeshFaceRendering.DoubleSided], meaning both front and back faces are shown.
    #[rerun(optional)]
    pub face_rendering: Option<rerun::components::MeshFaceRendering>,

    /// Optional albedo texture.
    ///
    /// Used with the [components.Texcoord2D] of the mesh.
    ///
    /// Currently supports only sRGB(A) textures, ignoring alpha.
    /// (meaning that the tensor must have 3 or 4 channels and use the `u8` format)
    ///
    /// The alpha channel is ignored.
    #[rerun(optional)]
    pub albedo_texture_buffer: Option<rerun::components::ImageBuffer>,

    /// The format of the `albedo_texture_buffer`, if any.
    #[rerun(optional)]
    pub albedo_texture_format: Option<rerun::components::ImageFormat>,

    /// Optional class Ids for the vertices.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
