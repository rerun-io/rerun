use enumset::__internal::EnumSetTypePrivate as _;
use enumset::EnumSet;
use re_log::{debug_assert, debug_panic};

use super::DrawPhase;
use crate::context::Renderers;
use crate::renderer::{
    DrawDataDrawable, DrawDataDrawablePayload, DrawInstruction, DrawableCollectionViewInfo,
};
use crate::{GpuRenderPipelinePoolAccessor, QueueableDrawData, RenderContext, RendererTypeId};

/// Draw data id within the [`DrawPhaseManager`].
type DrawDataIndex = u32;

/// Combined draw data index and rendering key.
///
/// This tightly packs the two values into a single u32 for sorting.
/// The renderer key forms the first 8 significant bits, the draw data the remaining 24.
/// This way, sorting the this value ascending, will sort the draw data by renderer and then by draw data index.
///
/// Note that a single [`DrawDataIndex`] can only ever refer to a single [`RendererTypeId`].
/// Therefore, we could alternatively pre-sort draw data by renderer so that the resulting
/// [`DrawDataIndex`] are already grouped by renderer.
/// However, using just the higher 8 bits for [`RendererTypeId`] makes the process a lot simpler.
/// We may reconsider this if we change the design such that variations of renderers are
/// expressed in the [`RendererTypeId`] such that 8 bit are no longer sufficient.
///
/// The packed [`RendererTypeId`] is session-local (assigned on registration); for deterministic
/// cross-session sort order, remap it via [`crate::context::Renderers::name_sort_remap`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PackedRenderingKeyAndDrawDataIndex(u32);

impl PackedRenderingKeyAndDrawDataIndex {
    #[inline]
    const fn new(renderer_key: RendererTypeId, draw_data_index: DrawDataIndex) -> Self {
        // 24 bits for the draw data index. Should be enough for anyone ;-).
        #![expect(clippy::disallowed_macros)] // Need to use the `std` macro in const contextss
        std::debug_assert!(draw_data_index < 0xFFFFFF);

        Self(((renderer_key.bits() as u32) << 24) | draw_data_index)
    }

    #[inline]
    const fn draw_data_index(&self) -> DrawDataIndex {
        self.0 & 0x00FFFFFF
    }

    #[inline]
    const fn renderer_key(&self) -> RendererTypeId {
        RendererTypeId::from_bits(((self.0 & 0xFF000000) >> 24) as u8)
    }
}

impl std::fmt::Debug for PackedRenderingKeyAndDrawDataIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackedRenderingKeyAndDrawDataIndex")
            .field("draw_data_index", &self.draw_data_index())
            .field("renderer_key", &self.renderer_key())
            .finish()
    }
}

/// A single drawable item within a given [`crate::renderer::DrawData`].
///
/// For more details see [`DrawDataDrawable`].
/// This is an expanded version used for processing/sorting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drawable {
    /// Distance sort key from near (low values) to far (high values).
    ///
    /// For draw phases that use camera distances, 0 is regarded as being at the camera
    /// with values increasing towards infinity with squared (!!) distance.
    /// However, all values from -INF to INF are valid.
    ///
    /// See also [`DrawDataDrawable::distance_sort_key`].
    pub distance_sort_key: f32,

    /// Secondary key used to consistently order otherwise similarly sorted drawables,
    /// e.g. draw order for coplanar drawables.
    pub secondary_sort_key: f32,

    /// Draw data index plus rendering key.
    draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex,

    /// See [`DrawDataDrawable::draw_data_payload`].
    pub draw_data_payload: DrawDataDrawablePayload,
}

impl Drawable {
    #[inline]
    fn draw_data_index(&self) -> DrawDataIndex {
        self.draw_data_plus_rendering_key.draw_data_index()
    }

    #[inline]
    fn renderer_key(&self) -> RendererTypeId {
        self.draw_data_plus_rendering_key.renderer_key()
    }

    /// Sort key for this drawable under the given name-sort remap.
    ///
    /// `name_sort_remap[key]` is the name-sorted rank of each registration-order
    /// [`RendererTypeId`] — see [`crate::context::Renderers::name_sort_remap`].
    #[inline]
    fn renderer_sort_key(&self, name_sort_remap: &[u8; 256]) -> u8 {
        name_sort_remap[self.renderer_key().bits() as usize]
    }

    /// Sorting key used for the opaque phases.
    ///
    /// Aggressively bundles by renderer type & draw data index.
    /// Within a single draw data, it puts near objects first so that the GPU can use early-z
    /// to discard objects that are further away.
    ///
    /// `name_sort_remap` provides a deterministic renderer ordering based on lexicographic
    /// comparison of Rust type names; this is what makes draw order stable across sessions.
    #[inline]
    fn sort_for_opaque_phase(drawables: &mut [Self], name_sort_remap: &[u8; 256]) {
        drawables.sort_by(|a, b| {
            a.renderer_sort_key(name_sort_remap)
                .cmp(&b.renderer_sort_key(name_sort_remap))
                .then_with(|| a.draw_data_index().cmp(&b.draw_data_index()))
                .then_with(|| a.distance_sort_key.total_cmp(&b.distance_sort_key))
                .then_with(|| a.secondary_sort_key.total_cmp(&b.secondary_sort_key))
        });
    }

    /// Sorting key used for transparent phases.
    ///
    /// Sorts far to near to facilitate blending.
    /// Since we're using the distance sort key above all else, there's no point in
    /// sorting by draw data index or renderer type at all since two [`Drawable::distance_sort_key`]
    /// are almost certainly going to be different.
    #[inline]
    fn sort_for_transparent_phase(drawables: &mut [Self]) {
        drawables.sort_by(|a, b| {
            b.distance_sort_key
                .total_cmp(&a.distance_sort_key)
                .then_with(|| a.secondary_sort_key.total_cmp(&b.secondary_sort_key))
        });
    }
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

    /// Sorts all drawables for all active phases.
    ///
    /// Renderer-type ordering is derived from `Renderers::name_sort_remap` so draw order
    /// is deterministic across sessions regardless of which renderer was registered first.
    pub fn sort_drawables(&mut self, renderers: &Renderers) {
        re_tracing::profile_function!();

        let name_sort_remap = renderers.name_sort_remap();

        // TODO(andreas): once we have traits/more dynamic interfaces for phases, they should own the sorting configuration.
        for phase in self.active_phases {
            if phase == DrawPhase::Transparent || phase == DrawPhase::OutlineMaskNoDepth {
                Drawable::sort_for_transparent_phase(&mut self.drawables[phase as usize]);
            } else {
                Drawable::sort_for_opaque_phase(
                    &mut self.drawables[phase as usize],
                    &name_sort_remap,
                );
            }
        }
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

        let renderer_chunked_drawables =
            self.drawables[phase as usize].chunk_by(|a, b| a.renderer_key() == b.renderer_key());

        // Re-use draw instruction array so we don't have to allocate all the time.
        let mut draw_instructions = Vec::with_capacity(64.min(self.draw_data.len()));

        for drawable_run_with_same_renderer in renderer_chunked_drawables {
            let first = &drawable_run_with_same_renderer[0]; // `std::slice::chunk_by` should always have at least one element per chunk.
            let renderer_key = first.renderer_key();

            // One instruction per draw data.
            draw_instructions.clear();
            draw_instructions.extend(
                drawable_run_with_same_renderer
                    .chunk_by(|a, b| a.draw_data_index() == b.draw_data_index())
                    .map(|drawables| DrawInstruction {
                        draw_data: &self.draw_data[drawables[0].draw_data_index() as usize],
                        drawables,
                    }),
            );

            let Some(renderer) = renderers.get_by_key(renderer_key) else {
                debug_panic!(
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

    /// Returns the drawables for the given phase.
    ///
    /// Used only for testing.
    #[cfg(test)]
    pub fn drawables_for_phase(&self, phase: DrawPhase) -> &[Drawable] {
        &self.drawables[phase as usize]
    }
}

/// Collector injected into [`crate::renderer::DrawData::collect_drawables`] in order to build up drawable list.
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

    #[inline]
    fn make_drawable(
        info: DrawDataDrawable,
        draw_data_index: DrawDataIndex,
        renderer_key: RendererTypeId,
    ) -> Drawable {
        Drawable {
            distance_sort_key: info.distance_sort_key,
            secondary_sort_key: info.secondary_sort_key,
            draw_data_payload: info.draw_data_payload,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                renderer_key,
                draw_data_index,
            ),
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
                drawables
                    .iter()
                    .map(|info| Self::make_drawable(*info, *draw_data_index, *renderer_key)),
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

    /// Add a single drawable to a single phase.
    ///
    /// Ignores any phase that isn't active.
    #[inline]
    pub fn add_drawable_for_phase(&mut self, phase: DrawPhase, drawable: DrawDataDrawable) {
        let Self {
            per_phase_drawables,
            draw_data_index,
            renderer_key,
        } = self;

        if per_phase_drawables.active_phases.contains(phase) {
            per_phase_drawables.drawables[phase.enum_into_u32() as usize].push(
                Self::make_drawable(drawable, *draw_data_index, *renderer_key),
            );
        }
    }

    /// Returns the phases that are currently active.
    ///
    /// This can be used as a performance optimization to avoid collecting drawables for phases that are not active.
    #[inline]
    pub fn active_phases(&self) -> EnumSet<DrawPhase> {
        self.per_phase_drawables.active_phases
    }
}

#[cfg(test)]
mod tests {
    use core::f32;

    use super::*;

    const RENDERER_0: RendererTypeId = RendererTypeId::from_bits(0);
    const RENDERER_2: RendererTypeId = RendererTypeId::from_bits(2);

    /// Identity remap: [`RendererTypeId`] `i` has sort key `i`. Used by tests that want to
    /// preserve the direct registration-order sort semantics the tests were originally written
    /// against.
    const IDENTITY_REMAP: [u8; 256] = {
        let mut remap = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            remap[i] = i as u8;
            i += 1;
        }
        remap
    };

    const TEST_DRAWABLES: [Drawable; 7] = [
        Drawable {
            distance_sort_key: 0.0,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_0, 0),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: 1.0,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_0, 1),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: 2.0,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_0, 1),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: f32::MAX,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_0, 0),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: f32::INFINITY,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_0, 0),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: 2.0001,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_2, 0),
            draw_data_payload: 0,
        },
        Drawable {
            distance_sort_key: 2.0001,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(RENDERER_2, 0),
            draw_data_payload: 1, // Same as previous, but has a different payload.
        },
    ];

    #[test]
    fn test_sort_for_opaque_phase() {
        let expected = vec![
            Drawable {
                distance_sort_key: 0.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: f32::MAX,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: f32::INFINITY,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 1.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 1,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 2.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 1,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 2.0001,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_2, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 2.0001,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_2, 0,
                ),
                draw_data_payload: 1, // Same as previous, but has a different payload.
            },
        ];

        {
            let mut drawables = TEST_DRAWABLES.to_vec();
            Drawable::sort_for_opaque_phase(&mut drawables, &IDENTITY_REMAP);
            assert_eq!(drawables, expected);
        }

        // Try again with reversed sequence.
        {
            let mut drawables = TEST_DRAWABLES.to_vec();
            drawables.reverse();

            // payload does not partake in sorting, therefore we have to re-reverse the order for the
            // items in the test sequence that are identical but have different payloads.
            drawables.swap(0, 1);

            Drawable::sort_for_opaque_phase(&mut drawables, &IDENTITY_REMAP);
            assert_eq!(drawables, expected);
        }
    }

    #[test]
    fn test_sort_for_transparent_phase() {
        let expected = vec![
            Drawable {
                distance_sort_key: f32::INFINITY,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: f32::MAX,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 2.0001,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_2, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 2.0001,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_2, 0,
                ),
                draw_data_payload: 1, // Same as previous, but has a different payload.
            },
            Drawable {
                distance_sort_key: 2.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 1,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 1.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 1,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 0.0,
                secondary_sort_key: 0.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
        ];

        {
            let mut drawables = TEST_DRAWABLES.to_vec();
            Drawable::sort_for_transparent_phase(&mut drawables);
            assert_eq!(drawables, expected);
        }

        // Try again with reversed sequence.
        {
            let mut drawables = TEST_DRAWABLES.to_vec();
            drawables.reverse();

            // payload does not partake in sorting, therefore we have to re-reverse the order for the
            // items in the test sequence that are identical but have different payloads.
            drawables.swap(0, 1);

            Drawable::sort_for_transparent_phase(&mut drawables);
            assert_eq!(drawables, expected);
        }
    }

    #[test]
    fn test_sort_for_transparent_phase_uses_secondary_sort_key_for_equal_distances() {
        let mut drawables = vec![
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 20.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 1,
            },
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 10.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
        ];

        Drawable::sort_for_transparent_phase(&mut drawables);

        assert_eq!(drawables[0].draw_data_payload, 0);
        assert_eq!(drawables[1].draw_data_payload, 1);
    }

    #[test]
    fn test_sort_for_transparent_phase_keeps_distance_priority_before_secondary_sort_key() {
        let mut drawables = vec![
            Drawable {
                distance_sort_key: 25.0,
                secondary_sort_key: 20.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 1,
            },
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 10.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
        ];

        Drawable::sort_for_transparent_phase(&mut drawables);

        assert_eq!(drawables[0].draw_data_payload, 1);
        assert_eq!(drawables[1].draw_data_payload, 0);
    }

    #[test]
    fn test_sort_for_transparent_phase_keeps_distance_priority_over_secondary_sort_key() {
        let mut drawables = vec![
            Drawable {
                distance_sort_key: 4.1,
                secondary_sort_key: 10.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 20.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 1,
            },
        ];

        Drawable::sort_for_transparent_phase(&mut drawables);

        assert_eq!(drawables[0].draw_data_payload, 0);
        assert_eq!(drawables[1].draw_data_payload, 1);
    }

    #[test]
    fn test_sort_for_opaque_phase_uses_secondary_sort_key_as_tie_breaker() {
        let mut drawables = vec![
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 20.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 1,
            },
            Drawable {
                distance_sort_key: 4.0,
                secondary_sort_key: 10.0,
                draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(
                    RENDERER_0, 0,
                ),
                draw_data_payload: 0,
            },
        ];

        Drawable::sort_for_opaque_phase(&mut drawables, &IDENTITY_REMAP);

        assert_eq!(drawables[0].draw_data_payload, 0);
        assert_eq!(drawables[1].draw_data_payload, 1);
    }

    /// Regression test: two drawables with identical distance/secondary sort keys but different
    /// [`RendererTypeId`]s must have a deterministic relative order determined by the remap, not
    /// by whichever key happens to have the lower u8 value.
    ///
    /// This is the exact scenario that previously caused view draw-order flake in a map view (trajectory
    /// linestrings vs annotation points): both had `distance_sort_key = f32::MAX` and both were
    /// in the opaque phase; their relative order was therefore decided by the registration-order
    /// renderer key, which varied between sessions.
    #[test]
    fn test_opaque_sort_is_invariant_under_remap_inversion() {
        let key_a = RendererTypeId::from_bits(0);
        let key_b = RendererTypeId::from_bits(1);

        let make_drawable = |key: RendererTypeId, payload: DrawDataDrawablePayload| Drawable {
            distance_sort_key: f32::MAX,
            secondary_sort_key: 0.0,
            draw_data_plus_rendering_key: PackedRenderingKeyAndDrawDataIndex::new(key, 0),
            draw_data_payload: payload,
        };

        // Remap that swaps keys 0 and 1 — as would happen if renderer B's type name sorts
        // before renderer A's.
        let mut swapped_remap = IDENTITY_REMAP;
        swapped_remap[0] = 1;
        swapped_remap[1] = 0;

        let base = vec![
            make_drawable(key_a, 0), // payload 0 = renderer A
            make_drawable(key_b, 1), // payload 1 = renderer B
        ];

        // Identity remap (A before B): A first.
        {
            let mut d = base.clone();
            Drawable::sort_for_opaque_phase(&mut d, &IDENTITY_REMAP);
            assert_eq!(d[0].draw_data_payload, 0);
            assert_eq!(d[1].draw_data_payload, 1);
        }

        // Swapped remap (B before A): B first — regardless of input order.
        {
            let mut d = base.clone();
            Drawable::sort_for_opaque_phase(&mut d, &swapped_remap);
            assert_eq!(d[0].draw_data_payload, 1);
            assert_eq!(d[1].draw_data_payload, 0);
        }

        // Order of the input vec must not affect the outcome — the remap is the only thing
        // that decides the winner.
        {
            let mut d = vec![make_drawable(key_b, 1), make_drawable(key_a, 0)];
            Drawable::sort_for_opaque_phase(&mut d, &swapped_remap);
            assert_eq!(d[0].draw_data_payload, 1);
            assert_eq!(d[1].draw_data_payload, 0);
        }
    }
}
