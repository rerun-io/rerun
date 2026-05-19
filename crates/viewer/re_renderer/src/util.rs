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

/// Computes per-axis mean and standard deviation from an iterator of `DVec3` values.
///
/// Uses f64 accumulators because the variance formula (`sum_sq/n - mean²`) computes a
/// small number as the difference of two large ones. With f32's ~7 digits of precision,
/// points centered far from the origin (e.g. around 10000 with spread ~1) lose nearly all
/// significant digits in that subtraction. f64's ~15 digits avoid this.
///
/// Returns `None` if fewer than 2 values are provided.
fn mean_and_sigma(values: impl Iterator<Item = glam::DVec3>) -> Option<(glam::DVec3, glam::DVec3)> {
    let mut count = 0u64;
    let mut sum = glam::DVec3::ZERO;
    let mut sum_sq = glam::DVec3::ZERO;

    for d in values {
        sum += d;
        sum_sq += d * d;
        count += 1;
    }

    if count < 2 {
        return None;
    }

    let n = count as f64;
    let mean = sum / n;
    let variance = (sum_sq / n - mean * mean).max(glam::DVec3::ZERO);
    let sigma = glam::DVec3::new(variance.x.sqrt(), variance.y.sqrt(), variance.z.sqrt());
    Some((mean, sigma))
}

/// Both the exact bounding box and a region of interest for a point cloud.
pub struct PointCloudBounds {
    /// Exact bounding box containing all finite points.
    pub bbox: macaw::BoundingBox,

    /// Region of interest that excludes spatial outliers.
    ///
    /// Useful for camera framing and other heuristics where extreme outliers
    /// should not dominate the view. For normally distributed data, covers ~95%
    /// of points; by Chebyshev's inequality, at least 75% for any distribution.
    pub region_of_interest: macaw::BoundingBox,
}

/// Computes both an exact bounding box and an outlier-robust region of interest
/// for a point cloud, using O(1) memory and two passes.
///
/// The region of interest is computed via a two-pass robust mean/σ approach:
/// **Pass 1**: Compute per-axis mean and standard deviation over all finite points.
/// **Pass 2**: Recompute mean and σ, ignoring points beyond 2σ from the initial mean.
/// The result is `[mean - 2σ, mean + 2σ]` from the cleaned statistics.
///
/// The second pass makes this robust against extreme outliers that would otherwise
/// skew the mean.
///
/// Non-finite points are ignored.
pub fn point_cloud_bounds(points: &[glam::Vec3]) -> PointCloudBounds {
    re_tracing::profile_function_if!(points.len() > 10000);

    let bbox = bounding_box_from_points(points.iter().copied());

    let finite_f64 = || {
        points
            .iter()
            .filter(|p| p.is_finite())
            .map(|p| p.as_dvec3())
    };

    // Pass 1: raw mean and σ over all finite points.
    let Some((mean, sigma)) = mean_and_sigma(finite_f64()) else {
        return PointCloudBounds {
            bbox,
            region_of_interest: bbox,
        };
    };

    // Pass 2: recompute, excluding points beyond 2σ from the raw mean.
    let lo = mean - 2.0 * sigma;
    let hi = mean + 2.0 * sigma;
    let Some((mean, sigma)) =
        mean_and_sigma(finite_f64().filter(|d| d.cmpge(lo).all() && d.cmple(hi).all()))
    else {
        return PointCloudBounds {
            bbox,
            region_of_interest: bbox,
        };
    };

    let region_of_interest = macaw::BoundingBox::from_min_max(
        (mean - 2.0 * sigma).as_vec3(),
        (mean + 2.0 * sigma).as_vec3(),
    );

    PointCloudBounds {
        bbox,
        region_of_interest,
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    #[test]
    fn point_cloud_bounds_excludes_outlier_from_region_of_interest() {
        // 9 points with varied x/y/z, clustered roughly in [0..3, 0..4, 0..5],
        // plus one outlier far away.
        let cluster_core = [
            Vec3::new(0.0, 1.0, 2.0),
            Vec3::new(1.0, 2.0, 0.5),
            Vec3::new(2.0, 0.5, 4.0),
            Vec3::new(0.5, 3.0, 1.0),
            Vec3::new(1.5, 1.5, 3.0),
            Vec3::new(0.2, 2.5, 0.8),
            Vec3::new(2.5, 0.2, 4.5),
            Vec3::new(0.8, 3.5, 2.5),
            Vec3::new(1.2, 1.8, 1.5),
        ];
        let outlier = Vec3::new(100.0, 200.0, 300.0);
        let points = cluster_core
            .iter()
            .copied()
            .chain(std::iter::once(outlier))
            .collect::<Vec<_>>();

        let bounds = point_cloud_bounds(&points);

        // The exact bbox must contain the outlier.
        assert!(
            bounds.bbox.contains(outlier),
            "bbox must contain outlier: {:?}",
            bounds.bbox,
        );

        // The ROI should NOT extend to the outlier.
        assert!(
            bounds.region_of_interest.max.x < 5.0
                && bounds.region_of_interest.max.y < 5.0
                && bounds.region_of_interest.max.z < 5.0
                && bounds.region_of_interest.min.x > -1.0
                && bounds.region_of_interest.min.y > -1.0
                && bounds.region_of_interest.min.z > -1.0,
            "outlier should not extend the region of interest: {:?}",
            bounds.region_of_interest,
        );

        // The ROI should still contain the bulk of the cluster.
        for point in cluster_core {
            assert!(
                bounds.region_of_interest.contains(point),
                "inlier point should be in region of interest: {point:?} in {:?}",
                bounds.region_of_interest,
            );
        }
    }
}
