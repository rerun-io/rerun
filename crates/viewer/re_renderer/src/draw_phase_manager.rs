use super::DrawPhase;

use enumset::__internal::EnumSetTypePrivate as _; // TODO: sounds fishy
use enumset::EnumSet;

use crate::{
    GpuRenderPipelinePoolAccessor, QueueableDrawData, RenderContext,
    context::Renderers,
    renderer::{
        DrawDataDrawable, DrawDataPayload, DrawInstruction, DrawableCollectionViewInfo,
        RendererTypeId,
    },
};

/// Darw data id within the [`DrawPhaseManager`].
type DrawDataIndex = u32;

#[derive(Debug, Clone, Copy)]

pub struct Drawable {
    pub distance_sort_key: f32,

    pub draw_data_payload: DrawDataPayload,

    draw_data_index: DrawDataIndex,

    /// Key for identifying the renderer type.
    renderer_key: RendererTypeId,
}

/// Manages the drawables for all active phases.
///
/// This is where collection & sorting of drawables and their underlying draw data happens.
/// Once all drawables are in place, we can render phase by phase.
pub struct DrawPhaseManager {
    active_phases: EnumSet<DrawPhase>,

    /// Drawables for all active phases.
    ///
    /// Since there's only a small, fixed number of phases,
    /// we can use a fixed size array, avoiding the need for a `HashMap`.
    drawables: [Vec<Drawable>; DrawPhase::VARIANT_COUNT as usize],

    draw_data: Vec<QueueableDrawData>,
}

impl DrawPhaseManager {
    /// Creates a new draw phase manager that takes care of planning drawing work for the given active phases.
    pub fn new(active_phases: EnumSet<DrawPhase>) -> Self {
        Self {
            active_phases,
            drawables: [const { Vec::new() }; DrawPhase::VARIANT_COUNT as usize],
            draw_data: Vec::new(),
        }
    }

    /// Adds a draw data to the draw phase manager.
    ///
    /// This will collect the drawables from the given draw data and add them to the appropriate work queues of each draw phase.
    pub fn add_draw_data(
        &mut self,
        ctx: &RenderContext,
        draw_data: QueueableDrawData,
        view_info: &DrawableCollectionViewInfo,
    ) {
        re_tracing::profile_function!();

        let draw_data_index = self.draw_data.len() as _;
        let renderer_key = draw_data.renderer_key(ctx);

        {
            let mut collector = DrawableCollector::new(self, draw_data_index, renderer_key);
            re_tracing::profile_scope!("collect_drawables");
            draw_data.collect_drawables(view_info, &mut collector);
        }

        self.draw_data.push(draw_data);
    }

    /// Draws all drawables for a given phase.
    // TODO(andreas): In the future this should also dispatch to specific phase setup & teardown which is right now hardcoded in `ViewBuilder`.
    pub fn draw(
        &self,
        renderers: &Renderers,
        gpu_resources: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        re_tracing::profile_function!(format!("draw({phase:?})"));

        debug_assert!(
            self.active_phases.contains(phase),
            "Phase {phase:?} not active",
        );

        // TODO(andreas): sort drawables according to the phases's requirements.

        let renderer_chunked_drawables =
            self.drawables[phase as usize].chunk_by(|a, b| a.renderer_key == b.renderer_key);

        // Re-use draw instruction array so we don't have to allocate all the time.
        let mut draw_instructions = Vec::with_capacity(64.min(self.draw_data.len()));

        for drawable_run_with_same_renderer in renderer_chunked_drawables {
            let first = &drawable_run_with_same_renderer[0]; // `std::slice::chunk_by` should always have at least one element per chunk.
            let renderer_key = first.renderer_key;

            // One instruction per draw data.
            draw_instructions.clear();
            draw_instructions.extend(
                drawable_run_with_same_renderer
                    .chunk_by(|a, b| a.draw_data_index == b.draw_data_index)
                    .map(|drawables| DrawInstruction {
                        draw_data: &self.draw_data[drawables[0].draw_data_index as usize],
                        drawables,
                    }),
            );

            let Some(renderer) = renderers.get_by_key(renderer_key) else {
                debug_assert!(
                    false,
                    "Previously acquired renderer not found by key. Since renderers are never deleted this should be impossible."
                );
                continue;
            };

            let draw_result =
                renderer.run_draw_instructions(gpu_resources, phase, pass, &draw_instructions);

            if let Err(err) = draw_result {
                re_log::error!("Error drawing with {}: {err}", renderer.name());
            }
        }
    }
}

/// Collector injected into [`DrawData::collect_drawables`] in order to build up drawable list.
pub struct DrawableCollector<'a> {
    per_phase_drawables: &'a mut DrawPhaseManager,
    draw_data_index: DrawDataIndex,
    renderer_key: RendererTypeId,
}

impl<'a> DrawableCollector<'a> {
    fn new(
        per_phase_drawables: &'a mut DrawPhaseManager,
        draw_data_index: DrawDataIndex,
        renderer_key: RendererTypeId,
    ) -> Self {
        Self {
            per_phase_drawables,
            draw_data_index,
            renderer_key,
        }
    }

    /// Add multiple drawables to the collector for the given phases.
    ///
    /// Ignores any phase that isn't active.
    #[inline]
    pub fn add_drawables(
        &mut self,
        phases: impl Into<EnumSet<DrawPhase>>,
        drawables: &[DrawDataDrawable],
    ) {
        let Self {
            per_phase_drawables,
            draw_data_index,
            renderer_key,
        } = self;

        let phases = per_phase_drawables
            .active_phases
            .intersection(phases.into());

        for phase in phases {
            per_phase_drawables.drawables[phase.enum_into_u32() as usize].extend(
                drawables.iter().map(|info| Drawable {
                    distance_sort_key: info.distance_sort_key,
                    draw_data_payload: info.draw_data_payload,
                    draw_data_index: *draw_data_index,
                    renderer_key: *renderer_key,
                }),
            );
        }
    }

    /// Add a single drawable to the collector for the given phases.
    ///
    /// Ignores any phase that isn't active.
    #[inline]
    pub fn add_drawable(
        &mut self,
        phases: impl Into<EnumSet<DrawPhase>>,
        drawable: DrawDataDrawable,
    ) {
        self.add_drawables(phases, &[drawable]);
    }

    /// Returns the phases that are currently active.
    ///
    /// This can be used as a performance optimization to avoid collecting drawables for phases that are not active.
    #[inline]
    pub fn active_phases(&self) -> EnumSet<DrawPhase> {
        self.per_phase_drawables.active_phases
    }
}
