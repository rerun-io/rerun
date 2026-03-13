/// Like [`macaw::BoundingBox::from_points`], but ignores NaN and infinity values.
pub fn bounding_box_from_points(points: impl Iterator<Item = glam::Vec3>) -> macaw::BoundingBox {
    let mut bbox = macaw::BoundingBox::nothing();
    for p in points {
        if p.is_finite() {
            bbox.extend(p);
        }
    }
    bbox
}
