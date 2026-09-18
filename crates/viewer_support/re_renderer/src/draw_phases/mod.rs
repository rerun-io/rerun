// TODO(andreas): The concept of DrawPhase implementers is very much in progress!
// Need to start to formalize this further and create implementers for all DrawPhases to build up our render graph.

mod depth_resolve;
mod draw_phase_manager;
mod outlines;
mod picking_layer;
mod screenshot;

pub use depth_resolve::DepthResolveProcessor;
pub use draw_phase_manager::{DrawPhaseManager, Drawable, DrawableCollector};
pub use outlines::{OutlineConfig, OutlineMaskPreference, OutlineMaskProcessor};
pub use picking_layer::{
    PickingLayerError, PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId,
    PickingLayerProcessor,
};
pub use screenshot::ScreenshotProcessor;

// ------------

/// Determines a (very rough) order of rendering and describes the active [`wgpu::RenderPass`].
///
/// Drawables are sorted back to front when [`Self::requires_back_to_front_sorting`] is true.
/// Other phases group drawables by renderer and draw data, then sort near objects first.
///
/// TODO(andreas): Should every phase/processor be associated with a single `wgpu::RenderPass`?
///     Note that this implies sub-phases (e.g. Opaque & background render to the same target).
///     Also we should then the higher level one to `RenderPass` or similar!
///
#[derive(Debug, enumset::EnumSetType)]
pub enum DrawPhase {
    /// Opaque objects, performing reads/writes to the depth buffer.
    ///
    /// Typically they are order independent, so everything uses this same index.
    Opaque = 0,

    /// Background, rendering where depth wasn't written.
    Background,

    /// Transparent objects, performing reads of the depth buffer, but no writes.
    Transparent,

    /// Volumes, sampling the opaque depth buffer without attaching it for rendering.
    Volume,

    /// Everything that can be picked with GPU based picking.
    ///
    /// Typically this contains everything from both the `Opaque` and `Transparent` phases drawn with z-test enabled.
    PickingLayer,

    /// Render mask for things that should get outlines.
    OutlineMask,

    /// Outline masks for special cases that should follow draw order
    /// instead of the regular [`DrawPhase::OutlineMask`] pass depth buffer.
    ///
    /// For example outlines for coplanar geometries that would otherwise have z-fighting.
    OutlineMaskNoDepth,

    /// Drawn when compositing with the main target.
    Compositing,

    /// Drawn when compositing with the main target, but for screenshots.
    /// This is a separate phase primarily because screenshots may be rendered with a different texture format.
    CompositingScreenshot,
}

impl DrawPhase {
    /// Whether drawables must be sorted back to front instead of grouped by renderer and draw data.
    pub fn requires_back_to_front_sorting(self) -> bool {
        match self {
            Self::Transparent | Self::Volume | Self::OutlineMaskNoDepth => true,
            Self::Opaque
            | Self::Background
            | Self::PickingLayer
            | Self::OutlineMask
            | Self::Compositing
            | Self::CompositingScreenshot => false,
        }
    }
}
