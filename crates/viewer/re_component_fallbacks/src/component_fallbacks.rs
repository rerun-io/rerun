use re_types::{Component as _, archetypes, components, datatypes};
use re_viewer_context::{
    ColormapWithRange, FallbackProviderRegistry, ImageDecodeCache, ImageInfo, ImageStatsCache,
    QueryContext, TensorStats, TensorStatsCache, ViewerContext, auto_color_for_entity_path,
};

pub fn type_fallbacks(registry: &mut FallbackProviderRegistry) {
    registry.register_type_fallback_provider::<components::Color>(|ctx| {
        auto_color_for_entity_path(ctx.target_entity_path)
    });
    registry.register_type_fallback_provider(|_| components::Radius::from(0.5));
    registry.register_type_fallback_provider(|_| components::HalfSize2D::from([0.5; 2]));
    registry.register_type_fallback_provider(|_| components::HalfSize3D::from([0.5; 3]));
    registry.register_type_fallback_provider(|_| components::Vector2D::from([1.0, 0.0]));
    registry.register_type_fallback_provider(|_| components::Vector3D::from([1.0, 0.0, 0.0]));
    registry.register_type_fallback_provider(|_| components::Timestamp::from(0));
    registry.register_type_fallback_provider(|_| components::SeriesVisible::from(true));
    registry.register_type_fallback_provider(|_| archetypes::Pinhole::DEFAULT_CAMERA_XYZ);
    registry.register_type_fallback_provider(|ctx| {
        // If the Pinhole has no resolution, use the resolution for the image logged at the same path.
        // See https://github.com/rerun-io/rerun/issues/3852
        resolution_of_image_at(ctx.viewer_ctx(), ctx.query, ctx.target_entity_path)
            // Zero will be seen as invalid resolution by the visualizer, making it opt out of visualization.
            // TODO(andreas): We should display a warning about this somewhere.
            // Since it's not a required component, logging a warning about this might be too noisy.
            .unwrap_or(components::Resolution::from([0.0, 0.0]))
    });
}

pub fn archetype_field_fallbacks(registry: &mut FallbackProviderRegistry) {
    // BarChart
    registry.register_fallback_provider(&archetypes::BarChart::descriptor_abscissa(), |ctx| {
        // This fallback is for abscissa - generate a sequence from 0 to n-1
        // where n is the length of the values tensor

        // Try to get the values tensor to determine the length
        if let Some(((_time, _row_id), tensor)) = ctx
            .recording()
            .latest_at_component::<components::TensorData>(
                ctx.target_entity_path,
                ctx.query,
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
    });

    // GraphNodes
    registry.register_fallback_provider(&archetypes::GraphNodes::descriptor_show_labels(), |_| {
        components::ShowLabels::from(true)
    });
    registry.register_fallback_provider(&archetypes::GraphNodes::descriptor_radii(), |_| {
        components::Radius::from(4.0)
    });

    // GeoLineStrings
    registry.register_fallback_provider(&archetypes::GeoLineStrings::descriptor_radii(), |_| {
        components::Radius::new_ui_points(2.0)
    });

    // GeoPoints
    registry.register_fallback_provider(&archetypes::GeoPoints::descriptor_radii(), |_| {
        components::Radius::new_ui_points(5.0)
    });

    // Arrows2D
    registry.register_fallback_provider(&archetypes::Arrows2D::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_LINES2D
    });
    registry.register_fallback_provider(&archetypes::Arrows2D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Arrows2D::descriptor_vectors(),
            &archetypes::Arrows2D::descriptor_labels(),
        )
    });

    // LineStrips2D
    registry.register_fallback_provider(&archetypes::LineStrips2D::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_LINES2D
    });
    registry.register_fallback_provider(
        &archetypes::LineStrips2D::descriptor_show_labels(),
        |ctx| {
            show_labels_fallback(
                ctx,
                &archetypes::LineStrips2D::descriptor_strips(),
                &archetypes::LineStrips2D::descriptor_labels(),
            )
        },
    );

    // Points2D
    registry.register_fallback_provider(&archetypes::Points2D::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_LINES2D
    });
    registry.register_fallback_provider(&archetypes::Points2D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Points2D::descriptor_positions(),
            &archetypes::Points2D::descriptor_labels(),
        )
    });

    // Arrows3D
    registry.register_fallback_provider(&archetypes::Arrows3D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Arrows3D::descriptor_vectors(),
            &archetypes::Arrows3D::descriptor_labels(),
        )
    });

    // LineStrips3D
    registry.register_fallback_provider(
        &archetypes::LineStrips3D::descriptor_show_labels(),
        |ctx| {
            show_labels_fallback(
                ctx,
                &archetypes::LineStrips3D::descriptor_strips(),
                &archetypes::LineStrips3D::descriptor_labels(),
            )
        },
    );

    // Points3D
    registry.register_fallback_provider(&archetypes::Points3D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Points3D::descriptor_positions(),
            &archetypes::Points3D::descriptor_labels(),
        )
    });

    // Boxes2D
    registry.register_fallback_provider(&archetypes::Boxes2D::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_BOX2D
    });
    registry.register_fallback_provider(&archetypes::Boxes2D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Boxes2D::descriptor_half_sizes(),
            &archetypes::Boxes2D::descriptor_labels(),
        )
    });

    // Boxes3D
    registry.register_fallback_provider(&archetypes::Boxes3D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Boxes3D::descriptor_half_sizes(),
            &archetypes::Boxes3D::descriptor_labels(),
        )
    });

    // Capsules3D
    registry.register_fallback_provider(&archetypes::Capsules3D::descriptor_show_labels(), |ctx| {
        show_labels_fallback(
            ctx,
            &archetypes::Capsules3D::descriptor_radii(),
            &archetypes::Capsules3D::descriptor_labels(),
        )
    });

    // Cylinders3D
    registry.register_fallback_provider(
        &archetypes::Cylinders3D::descriptor_show_labels(),
        |ctx| {
            show_labels_fallback(
                ctx,
                &archetypes::Cylinders3D::descriptor_radii(),
                &archetypes::Cylinders3D::descriptor_labels(),
            )
        },
    );

    // Ellipsoids3D
    registry.register_fallback_provider(
        &archetypes::Ellipsoids3D::descriptor_show_labels(),
        |ctx| {
            show_labels_fallback(
                ctx,
                &archetypes::Ellipsoids3D::descriptor_half_sizes(),
                &archetypes::Ellipsoids3D::descriptor_labels(),
            )
        },
    );

    // DepthImage
    registry.register_fallback_provider(&archetypes::DepthImage::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_DEPTH_IMAGE
    });
    registry.register_fallback_provider(&archetypes::DepthImage::descriptor_colormap(), |_| {
        ColormapWithRange::DEFAULT_DEPTH_COLORMAP
    });
    registry.register_fallback_provider(&archetypes::DepthImage::descriptor_meter(), |ctx| {
        let is_float_image = ctx
            .recording()
            .latest_at_component::<components::ImageFormat>(
                ctx.target_entity_path,
                ctx.query,
                archetypes::DepthImage::descriptor_format().component,
            )
            .is_some_and(|(_index, format)| format.is_float());

        components::DepthMeter::from(if is_float_image { 1.0 } else { 1000.0 })
    });
    registry.register_fallback_provider(&archetypes::DepthImage::descriptor_depth_range(), |ctx| {
        if let Some(((_time, buffer_row_id), image_buffer)) = ctx
            .recording()
            .latest_at_component::<components::ImageBuffer>(
                ctx.target_entity_path,
                ctx.query,
                archetypes::DepthImage::descriptor_buffer().component,
            )
        {
            // TODO(andreas): What about overrides on the image format?
            if let Some((_, format)) = ctx
                .recording()
                .latest_at_component::<components::ImageFormat>(
                    ctx.target_entity_path,
                    ctx.query,
                    archetypes::DepthImage::descriptor_format().component,
                )
            {
                let image = ImageInfo::from_stored_blob(
                    buffer_row_id,
                    archetypes::DepthImage::descriptor_buffer().component,
                    image_buffer.0,
                    format.0,
                    re_types::image::ImageKind::Depth,
                );
                let cache = ctx.store_ctx().caches;
                let image_stats = cache.entry(|c: &mut ImageStatsCache| c.entry(&image));
                let default_range = ColormapWithRange::default_range_for_depth_images(&image_stats);
                return [default_range[0] as f64, default_range[1] as f64].into();
            }
        }

        components::ValueRange::from([0.0, f64::MAX])
    });

    // EncodedImage
    registry.register_fallback_provider(&archetypes::EncodedImage::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_IMAGE
    });

    // Image
    registry.register_fallback_provider(&archetypes::Image::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_IMAGE
    });

    // SegmentationImage
    registry.register_fallback_provider(
        &archetypes::SegmentationImage::descriptor_draw_order(),
        |_| components::DrawOrder::DEFAULT_SEGMENTATION_IMAGE,
    );

    // VideoFrameReference
    registry.register_fallback_provider(
        &archetypes::VideoFrameReference::descriptor_draw_order(),
        |_| components::DrawOrder::DEFAULT_VIDEO,
    );

    // VideoStream
    registry.register_fallback_provider(&archetypes::VideoStream::descriptor_draw_order(), |_| {
        components::DrawOrder::DEFAULT_VIDEO
    });

    // Tensor
    registry.register_fallback_provider(&archetypes::Tensor::descriptor_value_range(), |ctx| {
        if let Some(((_time, row_id), tensor)) = ctx
            .recording()
            .latest_at_component::<components::TensorData>(
                ctx.target_entity_path,
                ctx.query,
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
    });

    // TextDocument
    registry.register_fallback_provider(&archetypes::TextDocument::descriptor_media_type(), |_| {
        components::MediaType::plain_text()
    });

    // SeriesLines
    registry.register_fallback_provider(&archetypes::SeriesLines::descriptor_widths(), |_| {
        components::StrokeWidth::from(0.75)
    });

    // SeriesPoints
    registry.register_fallback_provider(
        &archetypes::SeriesPoints::descriptor_marker_sizes(),
        |_| {
            // We use a larger default stroke width for scatter plots so the marker is
            // visible.
            components::MarkerSize::from(3.0)
        },
    );
}

fn resolution_of_image_at(
    ctx: &ViewerContext<'_>,
    query: &re_chunk_store::LatestAtQuery,
    entity_path: &re_log_types::EntityPath,
) -> Option<components::Resolution> {
    let entity_db = ctx.recording();
    let storage_engine = entity_db.storage_engine();

    // Check what kind of non-encoded images were logged here, if any.
    // TODO(andreas): can we do this more efficiently?
    // TODO(andreas): doesn't take blueprint into account!
    let all_components = storage_engine
        .store()
        .all_components_for_entity(entity_path)?;
    let image_format_descr = all_components
        .get(&archetypes::Image::descriptor_format().component)
        .or_else(|| all_components.get(&archetypes::DepthImage::descriptor_format().component))
        .or_else(|| {
            all_components.get(&archetypes::SegmentationImage::descriptor_format().component)
        });

    if let Some((_, image_format)) = image_format_descr.and_then(|component| {
        entity_db.latest_at_component::<components::ImageFormat>(entity_path, query, *component)
    }) {
        // Normal `Image` archetype
        return Some(components::Resolution::from([
            image_format.width as f32,
            image_format.height as f32,
        ]));
    }

    // Check for an encoded image.
    if let Some(((_time, row_id), blob)) = entity_db
        .latest_at_component::<re_types::components::Blob>(
            entity_path,
            query,
            archetypes::EncodedImage::descriptor_blob().component,
        )
    {
        let media_type = entity_db
            .latest_at_component::<components::MediaType>(
                entity_path,
                query,
                archetypes::EncodedImage::descriptor_media_type().component,
            )
            .map(|(_, c)| c);

        let image = ctx.store_context.caches.entry(|c: &mut ImageDecodeCache| {
            c.entry(
                row_id,
                archetypes::EncodedImage::descriptor_blob().component,
                &blob,
                media_type.as_ref(),
            )
        });

        if let Ok(image) = image {
            return Some(components::Resolution::from([
                image.format.width as f32,
                image.format.height as f32,
            ]));
        }
    }

    None
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
    instance_count_component: &re_types::ComponentDescriptor,
    text_component: &re_types::ComponentDescriptor,
) -> components::ShowLabels {
    debug_assert!(text_component.component_type == Some(components::Text::name()));

    let results = ctx.recording().latest_at(
        ctx.query,
        ctx.target_entity_path,
        [instance_count_component.component, text_component.component],
    );
    let num_instances = results
        .component_batch_raw(instance_count_component.component)
        .map_or(0, |array| array.len());
    let num_labels = results
        .component_batch_raw(text_component.component)
        .map_or(0, |array| array.len());

    components::ShowLabels::from(num_labels == 1 || num_instances < MAX_NUM_LABELS_PER_ENTITY)
}

/// Get a valid, finite range for the gpu to use.
fn tensor_data_range_heuristic(
    tensor_stats: &TensorStats,
    data_type: re_types::tensor_data::TensorDataType,
) -> components::ValueRange {
    let (min, max) = tensor_stats.finite_range;

    // Apply heuristic for ranges that are typically expected depending on the data type and the finite (!) range.
    // (we ignore NaN/Inf values heres, since they are usually there by accident!)
    #[expect(clippy::tuple_array_conversions)]
    components::ValueRange::from(if data_type.is_float() && 0.0 <= min && max <= 1.0 {
        // Float values that are all between 0 and 1, assume that this is the range.
        [0.0, 1.0]
    } else if 0.0 <= min && max <= 255.0 {
        // If all values are between 0 and 255, assume this is the range.
        // (This is very common, independent of the data type)
        [0.0, 255.0]
    } else if min == max {
        // uniform range. This can explode the colormapping, so let's map all colors to the middle:
        [min - 1.0, max + 1.0]
    } else {
        // Use range as is if nothing matches.
        [min, max]
    })
}
