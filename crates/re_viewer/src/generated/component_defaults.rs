use ::re_types_core::{external::arrow2, ComponentName, SerializationError};

/// Calls `default` for each component type in this module and serializes it to arrow. This is useful as a base fallback value when displaying ui.
#[allow(dead_code)]

pub fn list_default_components(
) -> Result<impl Iterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>, SerializationError>
{
    use ::re_types_core::{Loggable, LoggableBatch as _};
    re_tracing::profile_function!();
    Ok([
        (
            <super::ActiveTab as Loggable>::name(),
            super::ActiveTab::default().to_arrow()?,
        ),
        (
            <super::AutoLayout as Loggable>::name(),
            super::AutoLayout::default().to_arrow()?,
        ),
        (
            <super::AutoSpaceViews as Loggable>::name(),
            super::AutoSpaceViews::default().to_arrow()?,
        ),
        (
            <super::BackgroundKind as Loggable>::name(),
            super::BackgroundKind::default().to_arrow()?,
        ),
        (
            <super::ColumnShare as Loggable>::name(),
            super::ColumnShare::default().to_arrow()?,
        ),
        (
            <super::ContainerKind as Loggable>::name(),
            super::ContainerKind::default().to_arrow()?,
        ),
        (
            <super::Corner2D as Loggable>::name(),
            super::Corner2D::default().to_arrow()?,
        ),
        (
            <super::EntityPropertiesComponent as Loggable>::name(),
            super::EntityPropertiesComponent::default().to_arrow()?,
        ),
        (
            <super::GridColumns as Loggable>::name(),
            super::GridColumns::default().to_arrow()?,
        ),
        (
            <super::IncludedContent as Loggable>::name(),
            super::IncludedContent::default().to_arrow()?,
        ),
        (
            <super::IncludedSpaceView as Loggable>::name(),
            super::IncludedSpaceView::default().to_arrow()?,
        ),
        (
            <super::LockRangeDuringZoom as Loggable>::name(),
            super::LockRangeDuringZoom::default().to_arrow()?,
        ),
        (
            <super::PanelState as Loggable>::name(),
            super::PanelState::default().to_arrow()?,
        ),
        (
            <super::QueryExpression as Loggable>::name(),
            super::QueryExpression::default().to_arrow()?,
        ),
        (
            <super::RootContainer as Loggable>::name(),
            super::RootContainer::default().to_arrow()?,
        ),
        (
            <super::RowShare as Loggable>::name(),
            super::RowShare::default().to_arrow()?,
        ),
        (
            <super::SpaceViewClass as Loggable>::name(),
            super::SpaceViewClass::default().to_arrow()?,
        ),
        (
            <super::SpaceViewMaximized as Loggable>::name(),
            super::SpaceViewMaximized::default().to_arrow()?,
        ),
        (
            <super::SpaceViewOrigin as Loggable>::name(),
            super::SpaceViewOrigin::default().to_arrow()?,
        ),
        (
            <super::ViewerRecommendationHash as Loggable>::name(),
            super::ViewerRecommendationHash::default().to_arrow()?,
        ),
        (
            <super::Visible as Loggable>::name(),
            super::Visible::default().to_arrow()?,
        ),
        (
            <super::VisibleTimeRange as Loggable>::name(),
            super::VisibleTimeRange::default().to_arrow()?,
        ),
        (
            <super::VisualBounds2D as Loggable>::name(),
            super::VisualBounds2D::default().to_arrow()?,
        ),
        (
            <super::AnnotationContext as Loggable>::name(),
            super::AnnotationContext::default().to_arrow()?,
        ),
        (
            <super::Blob as Loggable>::name(),
            super::Blob::default().to_arrow()?,
        ),
        (
            <super::ClassId as Loggable>::name(),
            super::ClassId::default().to_arrow()?,
        ),
        (
            <super::ClearIsRecursive as Loggable>::name(),
            super::ClearIsRecursive::default().to_arrow()?,
        ),
        (
            <super::Color as Loggable>::name(),
            super::Color::default().to_arrow()?,
        ),
        (
            <super::DepthMeter as Loggable>::name(),
            super::DepthMeter::default().to_arrow()?,
        ),
        (
            <super::DisconnectedSpace as Loggable>::name(),
            super::DisconnectedSpace::default().to_arrow()?,
        ),
        (
            <super::DrawOrder as Loggable>::name(),
            super::DrawOrder::default().to_arrow()?,
        ),
        (
            <super::HalfSizes2D as Loggable>::name(),
            super::HalfSizes2D::default().to_arrow()?,
        ),
        (
            <super::HalfSizes3D as Loggable>::name(),
            super::HalfSizes3D::default().to_arrow()?,
        ),
        (
            <super::KeypointId as Loggable>::name(),
            super::KeypointId::default().to_arrow()?,
        ),
        (
            <super::LineStrip2D as Loggable>::name(),
            super::LineStrip2D::default().to_arrow()?,
        ),
        (
            <super::LineStrip3D as Loggable>::name(),
            super::LineStrip3D::default().to_arrow()?,
        ),
        (
            <super::MarkerShape as Loggable>::name(),
            super::MarkerShape::default().to_arrow()?,
        ),
        (
            <super::MarkerSize as Loggable>::name(),
            super::MarkerSize::default().to_arrow()?,
        ),
        (
            <super::Material as Loggable>::name(),
            super::Material::default().to_arrow()?,
        ),
        (
            <super::MediaType as Loggable>::name(),
            super::MediaType::default().to_arrow()?,
        ),
        (
            <super::Name as Loggable>::name(),
            super::Name::default().to_arrow()?,
        ),
        (
            <super::OutOfTreeTransform3D as Loggable>::name(),
            super::OutOfTreeTransform3D::default().to_arrow()?,
        ),
        (
            <super::PinholeProjection as Loggable>::name(),
            super::PinholeProjection::default().to_arrow()?,
        ),
        (
            <super::Position2D as Loggable>::name(),
            super::Position2D::default().to_arrow()?,
        ),
        (
            <super::Position3D as Loggable>::name(),
            super::Position3D::default().to_arrow()?,
        ),
        (
            <super::Radius as Loggable>::name(),
            super::Radius::default().to_arrow()?,
        ),
        (
            <super::Range1D as Loggable>::name(),
            super::Range1D::default().to_arrow()?,
        ),
        (
            <super::Resolution as Loggable>::name(),
            super::Resolution::default().to_arrow()?,
        ),
        (
            <super::Rotation3D as Loggable>::name(),
            super::Rotation3D::default().to_arrow()?,
        ),
        (
            <super::Scalar as Loggable>::name(),
            super::Scalar::default().to_arrow()?,
        ),
        (
            <super::ScalarScattering as Loggable>::name(),
            super::ScalarScattering::default().to_arrow()?,
        ),
        (
            <super::StrokeWidth as Loggable>::name(),
            super::StrokeWidth::default().to_arrow()?,
        ),
        (
            <super::TensorData as Loggable>::name(),
            super::TensorData::default().to_arrow()?,
        ),
        (
            <super::Texcoord2D as Loggable>::name(),
            super::Texcoord2D::default().to_arrow()?,
        ),
        (
            <super::Text as Loggable>::name(),
            super::Text::default().to_arrow()?,
        ),
        (
            <super::TextLogLevel as Loggable>::name(),
            super::TextLogLevel::default().to_arrow()?,
        ),
        (
            <super::Transform3D as Loggable>::name(),
            super::Transform3D::default().to_arrow()?,
        ),
        (
            <super::TriangleIndices as Loggable>::name(),
            super::TriangleIndices::default().to_arrow()?,
        ),
        (
            <super::Vector2D as Loggable>::name(),
            super::Vector2D::default().to_arrow()?,
        ),
        (
            <super::Vector3D as Loggable>::name(),
            super::Vector3D::default().to_arrow()?,
        ),
        (
            <super::ViewCoordinates as Loggable>::name(),
            super::ViewCoordinates::default().to_arrow()?,
        ),
        (
            <super::VisualizerOverrides as Loggable>::name(),
            super::VisualizerOverrides::default().to_arrow()?,
        ),
    ]
    .into_iter())
}
