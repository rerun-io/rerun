// TODO(andreas): The concept of DrawPhase implementers is very much in progress!
// Need to start to formalize this further and create implementers for all DrawPhases to build up our render graph.

mod draw_phase_manager;
mod outlines;
mod picking_layer;
mod screenshot;

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
/// Currently we do not support sorting *within* a rendering phase!
/// See [#702](https://github.com/rerun-io/rerun/issues/702)
/// Within a phase `DrawData` are drawn in the order they are submitted in.
///
/// TODO(andreas): Should every phase/processor be associated with a single `wgpu::RenderPass`?
///     Note that this implies sub-phases (e.g. Opaque & background render to the same target).
///     Also we should then the higher level one to `RenderPass` or similar!
///
// TODO(#1025, #4787): Add a 2D phase after Background and before Transparent which we can use
// to draw 2D objects that use a 2D layer key as sorting key
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

    /// Everything that can be picked with GPU based picking.
    ///
    /// Typically this contains everything from both the `Opaque` and `Transparent` phases drawn with z-test enabled.
    PickingLayer,

    /// Render mask for things that should get outlines.
    OutlineMask,

    /// Drawn when compositing with the main target.
    Compositing,

    /// Drawn when compositing with the main target, but for screenshots.
    /// This is a separate phase primarily because screenshots may be rendered with a different texture format.
    CompositingScreenshot,
}
