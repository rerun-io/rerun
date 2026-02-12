use re_sdk_types::{archetypes, components, datatypes};
use re_viewer_context::{
    ColormapWithRange, FallbackProviderRegistry, ImageDecodeCache, ImageInfo, ImageStatsCache,
    QueryContext, TensorStats, TensorStatsCache, auto_color_for_entity_path,
};

pub fn type_fallbacks(registry: &mut FallbackProviderRegistry) {
    registry.register_type_fallback_provider::<components::Color>(|ctx| {
        auto_color_for_entity_path(ctx.target_entity_path)
    });
    registry.register_type_fallback_provider(|_| archetypes::Pinhole::DEFAULT_CAMERA_XYZ);
    registry.register_type_fallback_provider(|ctx| {
        // If the Pinhole has no resolution, use the resolution for the image logged at the same path.
        // See https://github.com/rerun-io/rerun/issues/3852
        re_viewer_context::resolution_of_image_at(
            ctx.viewer_ctx(),
            &ctx.query,
            ctx.target_entity_path,
        )
        // Zero will be seen as invalid resolution by the visualizer, making it opt out of visualization.
        // TODO(andreas): We should display a warning about this somewhere.
        // Since it's not a required component, logging a warning about this might be too noisy.
        .unwrap_or_else(|| components::Resolution::from([0.0, 0.0]))
    });
}

pub fn archetype_field_fallbacks(registry: &mut FallbackProviderRegistry) {
    // BarChart
    registry.register_component_fallback_provider(
        archetypes::BarChart::descriptor_abscissa().component,
        |ctx| {
            // This fallback is for abscissa - generate a sequence from 0 to n-1
            // where n is the length of the values tensor

            // Try to get the values tensor to determine the length
            if let Some(((_time, _row_id), tensor)) = ctx
                .recording()
                .latest_at_component::<components::TensorData>(
                    ctx.target_entity_path,
                    &ctx.query,
                    archetypes::BarChart::descriptor_values().component,
                )
                && tensor.is_vector()
            {
                let shape = tensor.shape();
                if let Some(&length) = shape.first() {
                    // Create a sequence from 0 to length-1
                    #[expect(clippy::cast_possible_wrap)]
                    let indices: Vec<i64> = (0..length as i64).collect();
                    let tensor_data = datatypes::TensorData::new(
                        vec![length],
                        datatypes::TensorBuffer::I64(indices.into()),
                    );
                    return components::TensorData(tensor_data);
                }
            }

            // Fallback to empty tensor if we can't determine the values length
            let tensor_data =
                datatypes::TensorData::new(vec![0u64], datatypes::TensorBuffer::I64(vec![].into()));
            components::TensorData(tensor_data)
        },
    );
    registry.register_component_fallback_provider(
        archetypes::BarChart::descriptor_widths().component,
        |_| components::Length::from(1.0),
    );

    // GraphNodes
    registry.register_component_fallback_provider(
        archetypes::GraphNodes::descriptor_show_labels().component,
        |_| components::ShowLabels::from(true),
    );
    registry.register_component_fallback_provider(
        archetypes::GraphNodes::descriptor_radii().component,
        |_| components::Radius::from(4.0),
    );

    // GeoLineStrings
    registry.register_component_fallback_provider(
        archetypes::GeoLineStrings::descriptor_radii().component,
        |_| components::Radius::new_ui_points(2.0),
    );

    // GeoPoints
    registry.register_component_fallback_provider(
        archetypes::GeoPoints::descriptor_radii().component,
        |_| components::Radius::new_ui_points(5.0),
    );

    // Arrows2D
    registry.register_component_fallback_provider(
        archetypes::Arrows2D::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_LINES2D,
    );
    registry.register_component_fallback_provider(
        archetypes::Arrows2D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Arrows2D::descriptor_vectors().component,
                archetypes::Arrows2D::descriptor_labels().component,
            )
        },
    );

    // LineStrips2D
    registry.register_component_fallback_provider(
        archetypes::LineStrips2D::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_LINES2D,
    );
    registry.register_component_fallback_provider(
        archetypes::LineStrips2D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::LineStrips2D::descriptor_strips().component,
                archetypes::LineStrips2D::descriptor_labels().component,
            )
        },
    );

    // Points2D
    registry.register_component_fallback_provider(
        archetypes::Points2D::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_LINES2D,
    );
    registry.register_component_fallback_provider(
        archetypes::Points2D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Points2D::descriptor_positions().component,
                archetypes::Points2D::descriptor_labels().component,
            )
        },
    );

    // Arrows3D
    registry.register_component_fallback_provider(
        archetypes::Arrows3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Arrows3D::descriptor_vectors().component,
                archetypes::Arrows3D::descriptor_labels().component,
            )
        },
    );

    // LineStrips3D
    registry.register_component_fallback_provider(
        archetypes::LineStrips3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::LineStrips3D::descriptor_strips().component,
                archetypes::LineStrips3D::descriptor_labels().component,
            )
        },
    );

    // Points3D
    registry.register_component_fallback_provider(
        archetypes::Points3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Points3D::descriptor_positions().component,
                archetypes::Points3D::descriptor_labels().component,
            )
        },
    );

    // Boxes2D
    registry.register_component_fallback_provider(
        archetypes::Boxes2D::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_BOX2D,
    );
    registry.register_component_fallback_provider(
        archetypes::Boxes2D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Boxes2D::descriptor_half_sizes().component,
                archetypes::Boxes2D::descriptor_labels().component,
            )
        },
    );

    // Boxes3D
    registry.register_component_fallback_provider(
        archetypes::Boxes3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Boxes3D::descriptor_half_sizes().component,
                archetypes::Boxes3D::descriptor_labels().component,
            )
        },
    );

    // Capsules3D
    registry.register_component_fallback_provider(
        archetypes::Capsules3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Capsules3D::descriptor_radii().component,
                archetypes::Capsules3D::descriptor_labels().component,
            )
        },
    );

    // CoordinateFrame
    registry.register_component_fallback_provider(
        archetypes::CoordinateFrame::descriptor_frame().component,
        |ctx| components::TransformFrameId::from_entity_path(ctx.target_entity_path),
    );

    // Cylinders3D
    registry.register_component_fallback_provider(
        archetypes::Cylinders3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Cylinders3D::descriptor_radii().component,
                archetypes::Cylinders3D::descriptor_labels().component,
            )
        },
    );

    // Ellipsoids3D
    registry.register_component_fallback_provider(
        archetypes::Ellipsoids3D::descriptor_show_labels().component,
        |ctx| {
            show_labels_fallback(
                ctx,
                archetypes::Ellipsoids3D::descriptor_half_sizes().component,
                archetypes::Ellipsoids3D::descriptor_labels().component,
            )
        },
    );

    // DepthImage
    registry.register_component_fallback_provider(
        archetypes::DepthImage::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_DEPTH_IMAGE,
    );
    registry.register_component_fallback_provider(
        archetypes::DepthImage::descriptor_colormap().component,
        |_| ColormapWithRange::DEFAULT_DEPTH_COLORMAP,
    );
    registry.register_component_fallback_provider(
        archetypes::DepthImage::descriptor_meter().component,
        |ctx| {
            let is_float_image = ctx
                .recording()
                .latest_at_component::<components::ImageFormat>(
                    ctx.target_entity_path,
                    &ctx.query,
                    archetypes::DepthImage::descriptor_format().component,
                )
                .is_some_and(|(_index, format)| format.is_float());

            components::DepthMeter::from(if is_float_image { 1.0 } else { 1000.0 })
        },
    );
    registry.register_component_fallback_provider(
        archetypes::DepthImage::descriptor_depth_range().component,
        |ctx| {
            if let Some(((_time, buffer_row_id), image_buffer)) = ctx
                .recording()
                .latest_at_component::<components::ImageBuffer>(
                ctx.target_entity_path,
                &ctx.query,
                archetypes::DepthImage::descriptor_buffer().component,
            ) {
                // TODO(andreas): What about overrides on the image format?
                if let Some((_, format)) = ctx
                    .recording()
                    .latest_at_component::<components::ImageFormat>(
                        ctx.target_entity_path,
                        &ctx.query,
                        archetypes::DepthImage::descriptor_format().component,
                    )
                {
                    let image = ImageInfo::from_stored_blob(
                        buffer_row_id,
                        archetypes::DepthImage::descriptor_buffer().component,
                        image_buffer.0,
                        format.0,
                        re_sdk_types::image::ImageKind::Depth,
                    );
                    let cache = ctx.store_ctx().caches;
                    let image_stats = cache.entry(|c: &mut ImageStatsCache| c.entry(&image));
                    let default_range =
                        ColormapWithRange::default_range_for_depth_images(&image_stats);
                    return [default_range[0] as f64, default_range[1] as f64].into();
                }
            }

            components::ValueRange::from([0.0, f64::MAX])
        },
    );

    // EncodedDepthImage
    registry.register_component_fallback_provider(
        archetypes::EncodedDepthImage::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_DEPTH_IMAGE,
    );
    registry.register_component_fallback_provider(
        archetypes::EncodedDepthImage::descriptor_colormap().component,
        |_| ColormapWithRange::DEFAULT_DEPTH_COLORMAP,
    );
    registry.register_component_fallback_provider(
        archetypes::EncodedDepthImage::descriptor_depth_range().component,
        |ctx| {
            let blob = ctx.recording().latest_at_component::<components::Blob>(
                ctx.target_entity_path,
                &ctx.query,
                archetypes::EncodedDepthImage::descriptor_blob().component,
            );
            if let Some(((_time, row_id), blob)) = blob {
                let media_type = ctx
                    .recording()
                    .latest_at_component::<components::MediaType>(
                        ctx.target_entity_path,
                        &ctx.query,
                        archetypes::EncodedDepthImage::descriptor_media_type().component,
                    )
                    .map(|(_, media_type)| media_type);

                let cache = ctx.store_ctx().caches;
                let blob_bytes = blob.0.to_vec();
                if let Ok(image) = cache.entry(|c: &mut ImageDecodeCache| {
                    c.entry_encoded_depth(
                        row_id,
                        archetypes::EncodedDepthImage::descriptor_blob().component,
                        &blob_bytes,
                        media_type.as_ref(),
                    )
                }) {
                    let image_stats = cache.entry(|c: &mut ImageStatsCache| c.entry(&image));
                    let default_range =
                        ColormapWithRange::default_range_for_depth_images(&image_stats);
                    return [default_range[0] as f64, default_range[1] as f64].into();
                }
            }

            components::ValueRange::from([0.0, f64::MAX])
        },
    );

    // EncodedImage
    registry.register_component_fallback_provider(
        archetypes::EncodedImage::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_IMAGE,
    );

    // Image
    registry.register_component_fallback_provider(
        archetypes::Image::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_IMAGE,
    );

    // SegmentationImage
    registry.register_component_fallback_provider(
        archetypes::SegmentationImage::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_SEGMENTATION_IMAGE,
    );

    // VideoFrameReference
    registry.register_component_fallback_provider(
        archetypes::VideoFrameReference::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_VIDEO,
    );

    // VideoStream
    registry.register_component_fallback_provider(
        archetypes::VideoStream::descriptor_draw_order().component,
        |_| components::DrawOrder::DEFAULT_VIDEO,
    );

    // Tensor
    registry.register_component_fallback_provider(
        archetypes::Tensor::descriptor_value_range().component,
        |ctx| {
            if let Some(((_time, row_id), tensor)) = ctx
                .recording()
                .latest_at_component::<components::TensorData>(
                    ctx.target_entity_path,
                    &ctx.query,
                    archetypes::Tensor::descriptor_data().component,
                )
            {
                let tensor_stats = ctx.store_ctx().caches.entry(|c: &mut TensorStatsCache| {
                    c.entry(re_log_types::hash::Hash64::hash(row_id), &tensor)
                });
                tensor_data_range_heuristic(&tensor_stats, tensor.dtype())
            } else {
                components::ValueRange::new(0.0, 1.0)
            }
        },
    );

    // TextDocument
    registry.register_component_fallback_provider(
        archetypes::TextDocument::descriptor_media_type().component,
        |_| components::MediaType::plain_text(),
    );

    // Transform3D
    registry.register_component_fallback_provider(
        archetypes::Transform3D::descriptor_child_frame().component,
        |ctx| components::TransformFrameId::from_entity_path(ctx.target_entity_path),
    );
    registry.register_component_fallback_provider(
        archetypes::Transform3D::descriptor_parent_frame().component,
        |ctx| {
            components::TransformFrameId::from_entity_path(
                &ctx.target_entity_path
                    .parent()
                    .unwrap_or_else(re_log_types::EntityPath::root),
            )
        },
    );

    // Pinhole
    registry.register_component_fallback_provider(
        archetypes::Pinhole::descriptor_color().component,
        |ctx| components::Color::from(ctx.viewer_ctx().tokens().frustum_color),
    );
    registry.register_component_fallback_provider(
        archetypes::Pinhole::descriptor_line_width().component,
        |_| components::Radius::new_ui_points(1.),
    );
    registry.register_component_fallback_provider(
        archetypes::Pinhole::descriptor_child_frame().component,
        |ctx| components::TransformFrameId::from_entity_path(ctx.target_entity_path),
    );
    registry.register_component_fallback_provider(
        archetypes::Pinhole::descriptor_parent_frame().component,
        |ctx| {
            components::TransformFrameId::from_entity_path(
                &ctx.target_entity_path
                    .parent()
                    .unwrap_or_else(re_log_types::EntityPath::root),
            )
        },
    );

    // SeriesLines
    registry.register_component_fallback_provider(
        archetypes::SeriesLines::descriptor_widths().component,
        |_| components::StrokeWidth::from(0.75),
    );

    // SeriesPoints
    registry.register_component_fallback_provider(
        archetypes::SeriesPoints::descriptor_marker_sizes().component,
        |_| {
            // We use a larger default stroke width for scatter plots so the marker is
            // visible.
            components::MarkerSize::from(3.0)
        },
    );
}

/// Maximum number of labels after which we stop displaying labels for that entity all together,
/// unless overridden by a [`components::ShowLabels`] component.
const MAX_NUM_LABELS_PER_ENTITY: usize = 30;

/// Given a visualizer’s query context, compute its [`components::ShowLabels`] fallback value
/// (used when neither the logged data nor the blueprint provides a value).
///
/// Assumes that the visualizer reads the [`components::Text`] component for components.
/// The `instance_count_component` parameter must be the component descriptor that defines the number of instances
/// in the batch.
///
// TODO(kpreid): This component type (or the length directly) should be gotten from some kind of
// general mechanism of "how big is this batch?" rather than requiring the caller to specify it,
// possibly incorrectly.
fn show_labels_fallback(
    ctx: &QueryContext<'_>,
    instance_count_component: re_sdk_types::ComponentIdentifier,
    text_component: re_sdk_types::ComponentIdentifier,
) -> components::ShowLabels {
    let results = ctx.recording().latest_at(
        &ctx.query,
        ctx.target_entity_path,
        [instance_count_component, text_component],
    );
    let num_instances = results
        .component_batch_raw(instance_count_component)
        .map_or(0, |array| array.len());
    let num_labels = results
        .component_batch_raw(text_component)
        .map_or(0, |array| array.len());

    components::ShowLabels::from(num_labels == 1 || num_instances < MAX_NUM_LABELS_PER_ENTITY)
}

/// Get a valid, finite range for the gpu to use.
fn tensor_data_range_heuristic(
    tensor_stats: &TensorStats,
    data_type: re_sdk_types::tensor_data::TensorDataType,
) -> components::ValueRange {
    let (min, max) = re_viewer_context::gpu_bridge::data_range_heuristic(
        tensor_stats.finite_range,
        data_type.is_float(),
    );

    components::ValueRange::new(min, max)
}
