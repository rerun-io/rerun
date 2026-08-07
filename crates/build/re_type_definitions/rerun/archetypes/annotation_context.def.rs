// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The annotation context provides additional information on how to display entities.
///
/// Entities can use [components.ClassId]s and [components.KeypointId]s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// annotation context. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
///
/// See also [datatypes.ClassDescription].
///
/// \example archetypes/annotation_context_rects !api title="Rectangles" image="https://static.rerun.io/annotation_context_rects/9b446c36011ed30fce7dc6ed03d5fd9557460f70/1200w.png"
/// \example archetypes/annotation_context_segmentation title="Segmentation" image="https://static.rerun.io/annotation_context_segmentation/6c9e88fc9d44a08031cadd444c2e58a985cc1208/1200w.png""
/// \example archetypes/annotation_context_connections !api title="Connections" image="https://static.rerun.io/annotation_context_connections/4a8422bc154699c5334f574ff01b55c5cd1748e3/1200w.png"
#[rerun::rerun_type]
#[docs(view_types = "Spatial2DView, Spatial3DView")]
#[rerun(state = "unstable")]
#[rerun(visualizer_none)]
#[rust(derive(PartialEq))]
pub struct AnnotationContext {
    /// List of class descriptions, mapping class indices to class names, colors etc.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub context: rerun::components::AnnotationContext,
}
