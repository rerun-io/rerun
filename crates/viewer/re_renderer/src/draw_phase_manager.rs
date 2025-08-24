use super::DrawPhase;

use enumset::__internal::EnumSetTypePrivate as _; // TODO: sounds fishy
use enumset::EnumSet;

use crate::{
    GpuRenderPipelinePoolAccessor, QueueableDrawData,
    context::Renderers,
    renderer::{DrawDataDrawable, DrawDataDrawableKey, DrawableCollectionViewInfo},
};

// TODO: better mod name.

#[derive(Debug, Clone, Copy)]

pub struct Drawable {
    info: DrawDataDrawable,
    draw_data_key: DrawDataDrawableKey,
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
    // TODO: docs

    pub fn new(active_phases: EnumSet<DrawPhase>) -> Self {
        Self {
            active_phases,
            drawables: [const { Vec::new() }; DrawPhase::VARIANT_COUNT as usize],
            draw_data: Vec::new(),
        }
    }

    pub fn add_draw_data(
        &mut self,
        draw_data: QueueableDrawData,
        view_info: &DrawableCollectionViewInfo,
    ) {
        let draw_data_index = self.draw_data.len() as _;

        {
            let mut collector = DrawableCollector::new(self, draw_data_index);
            draw_data.collect_drawables(view_info, &mut collector);
        }

        self.draw_data.push(draw_data);
    }

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

        // TODO: sort drawables according to the phases's requirements.
        // TODO: Batch multiple draw data into a single renderer invocation.
        for draw_data in &self.draw_data {
            let res = draw_data.draw(renderers, gpu_resources, phase, pass);
            if let Err(err) = res {
                re_log::error!(renderer=%draw_data.renderer_name(), %err,
                    "renderer failed to draw");
            }
        }
    }
}

// TODO: docs
pub struct DrawableCollector<'a> {
    per_phase_drawables: &'a mut DrawPhaseManager,
    draw_data_index: u32,
    // TODO: do we need this as well?
    //renderer_key: u8,
}

impl<'a> DrawableCollector<'a> {
    fn new(per_phase_drawables: &'a mut DrawPhaseManager, draw_data_index: u32) -> Self {
        Self {
            per_phase_drawables,
            draw_data_index,
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
        } = self;

        let phases = per_phase_drawables
            .active_phases
            .intersection(phases.into());

        for phase in phases {
            per_phase_drawables.drawables[phase.enum_into_u32() as usize].extend(
                drawables.iter().map(|info| Drawable {
                    info: *info,
                    draw_data_key: *draw_data_index,
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
