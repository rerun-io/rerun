#![allow(unused_imports)]
#![allow(clippy::wildcard_imports)]

use re_entity_db::blueprint::components::*;
use re_types::blueprint::components::*;
use re_types::components::*;
use re_types_blueprint::blueprint::components::*;
use re_types_core::components::*;
use re_types_core::{external::arrow2, ComponentName, SerializationError};

/// Calls `default` for each component type in this module and serializes it to arrow. This is useful as a base fallback value when displaying ui.
pub fn list_default_components(
) -> Result<impl Iterator<Item = (ComponentName, Box<dyn arrow2::array::Array>)>, SerializationError>
{
    use ::re_types_core::{Loggable, LoggableBatch as _};
    re_tracing::profile_function!();
    Ok([
        (
            <ActiveTab as Loggable>::name(),
            ActiveTab::default().to_arrow()?,
        ),
        (
            <AutoLayout as Loggable>::name(),
            AutoLayout::default().to_arrow()?,
        ),
        (
            <AutoSpaceViews as Loggable>::name(),
            AutoSpaceViews::default().to_arrow()?,
        ),
        (
            <BackgroundKind as Loggable>::name(),
            BackgroundKind::default().to_arrow()?,
        ),
        (
            <ColumnShare as Loggable>::name(),
            ColumnShare::default().to_arrow()?,
        ),
        (
            <ContainerKind as Loggable>::name(),
            ContainerKind::default().to_arrow()?,
        ),
        (
            <Corner2D as Loggable>::name(),
            Corner2D::default().to_arrow()?,
        ),
        (
            <EntityPropertiesComponent as Loggable>::name(),
            EntityPropertiesComponent::default().to_arrow()?,
        ),
        (
            <GridColumns as Loggable>::name(),
            GridColumns::default().to_arrow()?,
        ),
        (
            <IncludedContent as Loggable>::name(),
            IncludedContent::default().to_arrow()?,
        ),
        (
            <IncludedSpaceView as Loggable>::name(),
            IncludedSpaceView::default().to_arrow()?,
        ),
        (
            <LockRangeDuringZoom as Loggable>::name(),
            LockRangeDuringZoom::default().to_arrow()?,
        ),
        (
            <PanelState as Loggable>::name(),
            PanelState::default().to_arrow()?,
        ),
        (
            <QueryExpression as Loggable>::name(),
            QueryExpression::default().to_arrow()?,
        ),
        (
            <RootContainer as Loggable>::name(),
            RootContainer::default().to_arrow()?,
        ),
        (
            <RowShare as Loggable>::name(),
            RowShare::default().to_arrow()?,
        ),
        (
            <SpaceViewClass as Loggable>::name(),
            SpaceViewClass::default().to_arrow()?,
        ),
        (
            <SpaceViewMaximized as Loggable>::name(),
            SpaceViewMaximized::default().to_arrow()?,
        ),
        (
            <SpaceViewOrigin as Loggable>::name(),
            SpaceViewOrigin::default().to_arrow()?,
        ),
        (
            <ViewerRecommendationHash as Loggable>::name(),
            ViewerRecommendationHash::default().to_arrow()?,
        ),
        (
            <Visible as Loggable>::name(),
            Visible::default().to_arrow()?,
        ),
        (
            <VisibleTimeRange as Loggable>::name(),
            VisibleTimeRange::default().to_arrow()?,
        ),
        (
            <VisualBounds2D as Loggable>::name(),
            VisualBounds2D::default().to_arrow()?,
        ),
        (
            <AnnotationContext as Loggable>::name(),
            AnnotationContext::default().to_arrow()?,
        ),
        (
            <AxisLength as Loggable>::name(),
            AxisLength::default().to_arrow()?,
        ),
        (<Blob as Loggable>::name(), Blob::default().to_arrow()?),
        (
            <ClassId as Loggable>::name(),
            ClassId::default().to_arrow()?,
        ),
        (
            <ClearIsRecursive as Loggable>::name(),
            ClearIsRecursive::default().to_arrow()?,
        ),
        (<Color as Loggable>::name(), Color::default().to_arrow()?),
        (
            <DepthMeter as Loggable>::name(),
            DepthMeter::default().to_arrow()?,
        ),
        (
            <DisconnectedSpace as Loggable>::name(),
            DisconnectedSpace::default().to_arrow()?,
        ),
        (
            <DrawOrder as Loggable>::name(),
            DrawOrder::default().to_arrow()?,
        ),
        (
            <HalfSizes2D as Loggable>::name(),
            HalfSizes2D::default().to_arrow()?,
        ),
        (
            <HalfSizes3D as Loggable>::name(),
            HalfSizes3D::default().to_arrow()?,
        ),
        (
            <ImagePlaneDistance as Loggable>::name(),
            ImagePlaneDistance::default().to_arrow()?,
        ),
        (
            <KeypointId as Loggable>::name(),
            KeypointId::default().to_arrow()?,
        ),
        (
            <LineStrip2D as Loggable>::name(),
            LineStrip2D::default().to_arrow()?,
        ),
        (
            <LineStrip3D as Loggable>::name(),
            LineStrip3D::default().to_arrow()?,
        ),
        (
            <MarkerShape as Loggable>::name(),
            MarkerShape::default().to_arrow()?,
        ),
        (
            <MarkerSize as Loggable>::name(),
            MarkerSize::default().to_arrow()?,
        ),
        (
            <Material as Loggable>::name(),
            Material::default().to_arrow()?,
        ),
        (
            <MediaType as Loggable>::name(),
            MediaType::default().to_arrow()?,
        ),
        (<Name as Loggable>::name(), Name::default().to_arrow()?),
        (
            <OutOfTreeTransform3D as Loggable>::name(),
            OutOfTreeTransform3D::default().to_arrow()?,
        ),
        (
            <PinholeProjection as Loggable>::name(),
            PinholeProjection::default().to_arrow()?,
        ),
        (
            <Position2D as Loggable>::name(),
            Position2D::default().to_arrow()?,
        ),
        (
            <Position3D as Loggable>::name(),
            Position3D::default().to_arrow()?,
        ),
        (<Radius as Loggable>::name(), Radius::default().to_arrow()?),
        (
            <Range1D as Loggable>::name(),
            Range1D::default().to_arrow()?,
        ),
        (
            <Resolution as Loggable>::name(),
            Resolution::default().to_arrow()?,
        ),
        (
            <Rotation3D as Loggable>::name(),
            Rotation3D::default().to_arrow()?,
        ),
        (<Scalar as Loggable>::name(), Scalar::default().to_arrow()?),
        (
            <StrokeWidth as Loggable>::name(),
            StrokeWidth::default().to_arrow()?,
        ),
        (
            <TensorData as Loggable>::name(),
            TensorData::default().to_arrow()?,
        ),
        (
            <Texcoord2D as Loggable>::name(),
            Texcoord2D::default().to_arrow()?,
        ),
        (<Text as Loggable>::name(), Text::default().to_arrow()?),
        (
            <TextLogLevel as Loggable>::name(),
            TextLogLevel::default().to_arrow()?,
        ),
        (
            <Transform3D as Loggable>::name(),
            Transform3D::default().to_arrow()?,
        ),
        (
            <TriangleIndices as Loggable>::name(),
            TriangleIndices::default().to_arrow()?,
        ),
        (
            <Vector2D as Loggable>::name(),
            Vector2D::default().to_arrow()?,
        ),
        (
            <Vector3D as Loggable>::name(),
            Vector3D::default().to_arrow()?,
        ),
        (
            <ViewCoordinates as Loggable>::name(),
            ViewCoordinates::default().to_arrow()?,
        ),
        (
            <VisualizerOverrides as Loggable>::name(),
            VisualizerOverrides::default().to_arrow()?,
        ),
    ]
    .into_iter())
}
