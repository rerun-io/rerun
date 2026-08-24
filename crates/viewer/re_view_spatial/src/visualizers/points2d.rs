use itertools::Itertools as _;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{ClassId, Color, KeypointId, Position2D, Radius, ShowLabels};
use re_sdk_types::{Archetype as _, ArrowString};
use re_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewClass as _, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem, typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::utilities::{LabeledBatch, process_labels_2d};
use crate::SpaceKind;
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::visualizers::{load_keypoint_connections, process_radius_slice};

// ---

#[derive(Default)]
pub struct Points2DVisualizer;

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Points2DVisualizer {
    fn process_data<'a>(
        view_data: &mut SpatialViewVisualizerData,
        ctx: &QueryContext<'_>,
        point_builder: &mut PointCloudBuilder<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Points2DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        // Opt-in due to the cost of CPU-sorting transparent point clouds every frame.
        let transparency_enabled = ctx
            .viewer_ctx()
            .app_options()
            .experimental
            .point_cloud_transparency;

        for data in data {
            let num_instances = data.positions.len();

            let positions = data
                .positions
                .iter()
                .map(|p| glam::vec3(p.x(), p.y(), 0.0))
                .collect_vec();

            let picking_ids = (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec();

            let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                positions.iter().copied(),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            let radii = process_radius_slice(
                ctx,
                entity_path,
                num_instances,
                data.radii,
                Points2D::descriptor_radii().component,
            );
            let colors = process_color_slice(
                ctx,
                Points2D::descriptor_colors().component,
                num_instances,
                &annotation_infos,
                data.colors,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, Points2D::name())
                .as_affine3a();

            let has_transparency = transparency_enabled && colors.iter().any(|c| !c.is_opaque());
            let robust_bounds = re_renderer::RobustBounds::from_points(&positions);

            {
                let point_batch = point_builder
                    .batch(entity_path.to_string())
                    .depth_offset(ent_context.depth_offset)
                    .flags(
                        re_renderer::renderer::PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES
                            | re_renderer::renderer::PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                    )
                    .enable_alpha_blending(has_transparency)
                    .world_from_obj(world_from_obj)
                    .object_space_bounding_box(robust_bounds.exact)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                let mut point_range_builder =
                    point_batch.add_points_2d(&positions, &radii, &colors, &picking_ids);

                // Determine if there's any sub-ranges that need extra highlighting.
                {
                    re_tracing::profile_scope!("marking additional highlight points");
                    #[expect(clippy::iter_over_hash_type)]
                    // Non-overlapping per-instance mask ranges.
                    for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                        let highlighted_point_index = (highlighted_key.get()
                            < num_instances as u64)
                            .then_some(highlighted_key.get());
                        if let Some(highlighted_point_index) = highlighted_point_index {
                            point_range_builder = point_range_builder
                                .push_additional_outline_mask_ids_for_range(
                                    re_span::Span::from_start_len(
                                        highlighted_point_index as u32,
                                        1,
                                    ),
                                    *instance_mask_ids,
                                );
                        }
                    }
                }
            }

            view_data.add_bounds(
                entity_path.hash(),
                robust_bounds,
                world_from_obj,
                SpaceKind::TwoD,
            );

            load_keypoint_connections(
                line_builder,
                &ent_context.annotations,
                world_from_obj,
                entity_path,
                &keypoints,
            )?;

            view_data.ui_labels.extend(process_labels_2d(
                LabeledBatch {
                    entity_path,
                    visualizer_instruction: ent_context.visualizer_instruction,
                    num_instances,
                    overall_position: robust_bounds.exact.center().truncate(),
                    instance_positions: data.positions.iter().map(|p| glam::vec2(p.x(), p.y())),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, Points2D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }

        Ok(())
    }
}

// ---

#[doc(hidden)] // Public for benchmarks
pub struct Points2DComponentData<'a> {
    // Point of views
    pub positions: &'a [Position2D],

    // Clamped to edge
    pub colors: &'a [Color],
    pub radii: &'a [Radius],
    pub labels: Vec<ArrowString>,
    pub keypoint_ids: &'a [KeypointId],
    pub class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Points2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        re_viewer_context::external::re_string_interner::intern_static!(
            re_viewer_context::ViewSystemIdentifier,
            "Points2D"
        )
    }
}

impl VisualizerSystem for Points2DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::single_required_component::<Position2D>(
            &Points2D::descriptor_positions(),
            &Points2D::all_components(),
        )
    }

    fn affinity(&self) -> Option<re_sdk_types::ViewClassIdentifier> {
        Some(crate::SpatialView2D::identifier())
    }

    fn execute(
        &self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut view_data = SpatialViewVisualizerData::default();
        let output = VisualizerExecutionOutput::default();

        let mut point_builder = PointCloudBuilder::new(ctx.viewer_ctx.render_ctx());
        point_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll
        // go with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Points2D, _, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self,
            |ctx, spatial_ctx, results| {
                let all_positions =
                    results.iter_required(Points2D::descriptor_positions().component);
                if all_positions.is_empty() {
                    return Ok(());
                }

                let num_positions = all_positions
                    .chunks()
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 2]>())
                    .map(|points| points.len())
                    .sum();

                if num_positions == 0 {
                    return Ok(());
                }

                point_builder.reserve(num_positions)?;
                let all_colors = results.iter_optional(Points2D::descriptor_colors().component);
                let all_radii = results.iter_optional(Points2D::descriptor_radii().component);
                let all_labels = results.iter_optional(Points2D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_optional(Points2D::descriptor_class_ids().component);
                let all_keypoint_ids =
                    results.iter_optional(Points2D::descriptor_keypoint_ids().component);
                let all_show_labels =
                    results.iter_optional(Points2D::descriptor_show_labels().component);

                let results_iter = re_query::range_zip_1x6(
                    all_positions.slice::<[f32; 2]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_keypoint_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(
                        _index,
                        positions,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                        show_labels,
                    )| {
                        Points2DComponentData {
                            positions: bytemuck::cast_slice(positions),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                Self::process_data(
                    &mut view_data,
                    ctx,
                    &mut point_builder,
                    &mut line_builder,
                    view_query,
                    spatial_ctx,
                    results_iter,
                )
            },
        )?;

        Ok(output
            .with_draw_data([
                point_builder.into_draw_data()?.into(),
                line_builder.into_draw_data()?.into(),
            ])
            .with_visualizer_data(view_data))
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::TimeInt;
    use re_sdk_types::archetypes::{AnnotationContext, Points2D};
    use re_sdk_types::blueprint::{
        archetypes::VisibleTimeRanges, components as blueprint_components,
    };
    use re_sdk_types::components::{ClassId, Color};
    use re_sdk_types::datatypes::{self, TimeRange, TimeRangeBoundary, VisibleTimeRange};
    use re_test_context::TestContext;
    use re_test_viewport::TestContextExt as _;
    use re_viewer_context::{ViewClass as _, ViewId};
    use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

    use crate::visualizers::{UiLabel, UiLabelStyle, collect_ui_labels};

    const ANN: Color = Color(datatypes::Rgba32::from_rgb(30, 60, 90));
    const LATE_ANN: Color = Color(datatypes::Rgba32::from_rgb(90, 60, 30));
    const RED: Color = Color(datatypes::Rgba32::from_rgb(255, 0, 0));
    const GREEN: Color = Color(datatypes::Rgba32::from_rgb(0, 255, 0));
    const BLUE: Color = Color(datatypes::Rgba32::from_rgb(0, 0, 255));

    fn setup_entities(ctx: &mut TestContext) {
        let timeline = re_log_types::Timeline::new_sequence("frame");

        ctx.log_entity("/", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 1)],
                &AnnotationContext::new([(3, "three", ANN.0)]),
            )
        });
        // Add another annotation context at a later time to test that the latest-at resolution works.
        ctx.log_entity("/", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 5)],
                &AnnotationContext::new([(3, "three-updated", LATE_ANN.0)]),
            )
        });

        fn positions(y: f32) -> [(f32, f32); 3] {
            [(10.0, y), (20.0, y), (30.0, y)]
        }

        // One concrete Points2D entity, queried both latest-at and as a time range.
        // Labels are enabled so the resolved color/label can be inspected without image snapshots.
        //
        // `omitted` means the component should carry its previous value forward.
        // `[]` means an explicit empty component batch, which should reset the previous value.
        //
        // frame:      1             2                 3          4                  5
        // colors:     R/G/B         omitted           []         omitted            blue
        // class IDs:  omitted       3                 omitted    []                 3
        // expected: manual RGB; annotation label and RGB; annotation 3; manual label only; annotation label and blue
        ctx.log_entity("points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 1)],
                &Points2D::new(positions(10.0))
                    .with_show_labels(true)
                    .with_colors([RED, GREEN, BLUE])
                    .with_labels(["manual-red", "manual-green", "manual-blue"]),
            )
        });
        ctx.log_entity("points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 2)],
                &Points2D::new(positions(20.0))
                    .with_class_ids([3])
                    .with_labels([] as [&str; 0]),
            )
        });
        ctx.log_entity("points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 3)],
                &Points2D::new(positions(30.0))
                    .with_colors([] as [Color; 0])
                    .with_labels([] as [&str; 0]),
            )
        });
        ctx.log_entity("points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 4)],
                &Points2D::new(positions(40.0))
                    .with_class_ids([] as [ClassId; 0])
                    .with_labels(["manual-label-after-class-reset"]),
            )
        });
        ctx.log_entity("points", |builder| {
            builder.with_archetype_auto_row(
                [(timeline, 5)],
                &Points2D::new(positions(50.0))
                    .with_colors([BLUE])
                    .with_class_ids([3])
                    .with_labels([] as [&str; 0]),
            )
        });

        ctx.set_active_timeline(*timeline.name());
    }

    fn collect_labels(test_context: &TestContext, view_id: ViewId) -> Vec<UiLabel> {
        let labels = std::cell::RefCell::new(Vec::new());
        let mut harness = test_context
            .setup_kittest_for_rendering_3d(egui::vec2(1.0, 1.0))
            .build_ui(|ui| {
                test_context.handle_system_commands(ui.ctx());
                test_context.run_ui(ui, |ctx, _ui| {
                    let view = ViewBlueprint::try_from_db(
                        view_id,
                        ctx.store_context.blueprint,
                        ctx.blueprint_query,
                    )
                    .expect("view should exist");

                    let registry = ctx.view_class_registry();
                    let class = registry.get_class_or_log_error(view.class_identifier());
                    let once_per_frame = registry.run_once_per_frame_context_systems(
                        ctx,
                        std::iter::once(view.class_identifier()),
                    );
                    let mut view_states = test_context.view_states.lock();
                    let view_state = view_states.get_mut_or_create(ctx.store_id(), view_id, class);
                    let (_, output) = re_viewport::execute_systems_for_view(
                        ctx,
                        &view,
                        view_state,
                        &once_per_frame,
                    );

                    *labels.borrow_mut() = collect_ui_labels(&output);
                });
            });
        harness.run();
        drop(harness);

        labels.into_inner()
    }

    fn assert_labels(labels: &[UiLabel], expected: &[(&str, Color)]) {
        assert_eq!(expected.len(), labels.len());
        for (label, (expected_text, expected_color)) in std::iter::zip(labels.iter(), expected) {
            assert_eq!(*expected_text, label.text);
            assert!(label.style == UiLabelStyle::Color((*expected_color).into()));
        }
    }

    #[test]
    fn color_resolution_mixes_manual_colors_and_annotation_context_latest_at() {
        let mut test_context = TestContext::new_with_view_class::<crate::SpatialView2D>();
        setup_entities(&mut test_context);

        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                crate::SpatialView2D::identifier(),
            ))
        });

        test_context.set_time(1);
        assert_labels(
            &collect_labels(&test_context, view_id),
            &[
                ("manual-red", RED),
                ("manual-green", GREEN),
                ("manual-blue", BLUE),
            ],
        );

        test_context.set_time(2);
        assert_labels(
            &collect_labels(&test_context, view_id),
            &[("three", RED), ("three", GREEN), ("three", BLUE)],
        );

        test_context.set_time(3);
        assert_labels(
            &collect_labels(&test_context, view_id),
            &[("three", ANN); 3],
        );

        test_context.set_time(4);
        let no_class_id = collect_labels(&test_context, view_id);
        assert_eq!(1, no_class_id.len());
        assert_eq!("manual-label-after-class-reset", no_class_id[0].text);
        assert!(no_class_id[0].style != UiLabelStyle::Color(ANN.into()));

        test_context.set_time(5);
        assert_labels(
            &collect_labels(&test_context, view_id),
            &[("three-updated", BLUE); 3],
        );
    }

    #[test]
    fn color_resolution_mixes_manual_colors_and_annotation_context_range() {
        let mut test_context = TestContext::new_with_view_class::<crate::SpatialView2D>();
        setup_entities(&mut test_context);

        let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
            let view_id = blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
                crate::SpatialView2D::identifier(),
            ));

            let property = ViewProperty::from_archetype_for_view::<VisibleTimeRanges>(ctx, view_id);
            property.save_blueprint_component(
                ctx,
                &VisibleTimeRanges::descriptor_ranges(),
                &blueprint_components::VisibleTimeRange(VisibleTimeRange {
                    timeline: "frame".into(),
                    range: TimeRange {
                        start: TimeRangeBoundary::Absolute(
                            TimeInt::from_sequence(1.try_into().unwrap()).into(),
                        ),
                        end: TimeRangeBoundary::Absolute(
                            TimeInt::from_sequence(5.try_into().unwrap()).into(),
                        ),
                    },
                }),
            );

            view_id
        });

        // Range covers the end-to-end range-zip path: optional colors and class IDs must carry
        // forward when omitted and reset when logged as empty, independently of one another.
        // The annotation context is resolved latest-at the cursor and applies to the entire range.
        test_context.set_time(3);
        let range = collect_labels(&test_context, view_id);
        assert_eq!(13, range.len());

        let expected_before_class_reset = [
            // 1
            ("manual-red", RED),
            ("manual-green", GREEN),
            ("manual-blue", BLUE),
            // 2
            ("three", RED),
            ("three", GREEN),
            ("three", BLUE),
            // 3
            ("three", ANN),
            ("three", ANN),
            ("three", ANN),
        ];
        assert_labels(&range[..9], &expected_before_class_reset);

        // Don't hard-code the fallback color after the class-id reset: the important behavior is
        // that the annotation label/color no longer applies, not which fallback color is chosen.
        assert_eq!("manual-label-after-class-reset", range[9].text);
        assert!(range[9].style != UiLabelStyle::Color(ANN.into()));

        assert_labels(&range[10..], &[("three", BLUE); 3]);

        test_context.set_time(5);
        let range_after_context_update = collect_labels(&test_context, view_id);
        assert_eq!(13, range_after_context_update.len());
        for index in [3, 4, 5, 6, 7, 8, 10, 11, 12] {
            assert_eq!("three-updated", range_after_context_update[index].text);
        }
        for label in &range_after_context_update[6..9] {
            assert!(label.style == UiLabelStyle::Color(LATE_ANN.into()));
        }
    }
}
