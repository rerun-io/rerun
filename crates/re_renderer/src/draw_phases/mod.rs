// TODO(andreas): The concept of DrawPhase implementors is very much in progress!
// Need to start to formalize this further and create implementors for all DrawPhases to build up our render graph.

mod outlines;
pub use outlines::{OutlineConfig, OutlineMaskPreference, OutlineMaskProcessor};

mod picking_layer;
pub use picking_layer::{
    PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId, PickingLayerProcessor,
    ScheduledPickingRect,
};

/// Determines a (very rough) order of rendering and describes the active [`wgpu::RenderPass`].
///
/// Currently we do not support sorting *within* a rendering phase!
/// See [#702](https://github.com/rerun-io/rerun/issues/702)
/// Within a phase `DrawData` are drawn in the order they are submitted in.
///
/// TODO(andreas): Should every phase/processor be associated with a single `wgpu::RenderPass`?
///     Note that this implies sub-phases (e.g. Opaque & background render to the same target).
///     Also we should then the higher level one to `RenderPass` or similar!
#[derive(Debug, enumset::EnumSetType)]
pub enum DrawPhase {
    /// Opaque objects, performing reads/writes to the depth buffer.
    ///
    /// Typically they are order independent, so everything uses this same index.
    Opaque,

    /// Background, rendering where depth wasn't written.
    Background,

    /// Everything that can be picked with GPU based picking.
    ///
    /// This should be everything in the `Opaque` phase.
    PickingLayer,

    /// Render mask for things that should get outlines.
    OutlineMask,

    /// Drawn when compositing with the main target.
    Compositing,

    /// Drawn when compositing with the main target, but for screenshots.
    /// This is a separate phase primarily because screenshots may be rendered with a different texture format.
    CompositingScreenshot,
}
