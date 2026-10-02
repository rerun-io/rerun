/// Like [`re_math::BoundingBox::from_points`], but ignores NaN and infinity values.
pub fn bounding_box_from_points(
    points: impl ExactSizeIterator<Item = glam::Vec3>,
) -> re_math::BoundingBox {
    re_tracing::profile_function_if!(10_000 < points.len());

    let mut bbox = re_math::BoundingBox::nothing();
    for p in points {
        if p.is_finite() {
            bbox.extend(p);
        }
    }
    bbox
}
