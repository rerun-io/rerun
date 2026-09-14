use crate::util::bounding_box_from_points;

/// Computes the per-axis mean and standard deviation of all finite points passing `keep`.
///
/// Uses f64 accumulators because the variance formula (`sum_sq/n - mean²`) computes a
/// small number as the difference of two large ones. With f32's ~7 digits of precision,
/// points centered far from the origin (e.g. around 10000 with spread ~1) lose nearly all
/// significant digits in that subtraction. f64's ~15 digits avoid this.
///
/// Returns `None` if fewer than 2 points are kept.
fn mean_and_sigma(
    points: &[glam::Vec3],
    keep: impl Fn(glam::DVec3) -> bool,
) -> Option<(glam::DVec3, glam::DVec3)> {
    re_tracing::profile_function_if!(10_000 < points.len());

    let mut count = 0u64;
    let mut sum = glam::DVec3::ZERO;
    let mut sum_sq = glam::DVec3::ZERO;

    for point in points {
        if !point.is_finite() {
            continue;
        }
        let d = point.as_dvec3();
        if !keep(d) {
            continue;
        }

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

/// An exact bounding box, plus a region of interest that is robust against outliers.
#[derive(Clone, Copy, Debug, re_byte_size::SizeBytes)]
pub struct RobustBounds {
    /// The exact bounding box, containing everything.
    pub exact: macaw::BoundingBox,

    /// Region of interest that excludes spatial outliers.
    ///
    /// Useful for camera framing and other heuristics where extreme outliers
    /// should not dominate the view. For normally distributed data, covers ~95%
    /// of points; by Chebyshev's inequality, at least 75% for any distribution.
    ///
    /// This is a statistical estimate, not a subset of [`Self::exact`]:
    /// it can be *larger* than the exact bounding box, depending on the distribution.
    /// For instance, uniformly distributed points have σ = extent/√12,
    /// so `mean ± 2σ` reaches ~1.15× the actual extent.
    /// Only heavy-tailed distributions (a dense cluster plus far-away outliers)
    /// give a region of interest smaller than the exact box.
    pub region_of_interest: macaw::BoundingBox,
}

impl RobustBounds {
    /// A bounding box without any outliers, i.e. the region of interest is the whole box.
    #[inline]
    pub fn from_bbox(bbox: macaw::BoundingBox) -> Self {
        Self {
            exact: bbox,
            region_of_interest: bbox,
        }
    }

    /// Computes both an exact bounding box and an outlier-robust region of interest
    /// for a set of points, using O(1) memory and two passes.
    ///
    /// The region of interest is computed via a two-pass robust mean/σ approach:
    /// **Pass 1**: Compute per-axis mean and standard deviation over all finite points.
    /// **Pass 2**: Recompute mean and σ, ignoring points beyond 2σ from the initial mean.
    /// The result is `[mean - 2σ, mean + 2σ]` from the cleaned statistics.
    ///
    /// The second pass makes this robust against extreme outliers that would otherwise
    /// skew the mean.
    ///
    /// Note that the region of interest is not clamped to the exact bounding box,
    /// and so may be larger than it — see [`Self::region_of_interest`].
    ///
    /// Non-finite points are ignored.
    pub fn from_points(points: &[glam::Vec3]) -> Self {
        re_tracing::profile_function_if!(10_000 < points.len());

        let exact = bounding_box_from_points(points.iter().copied());

        // Pass 1: raw mean and σ over all finite points.
        let Some((mean, sigma)) = mean_and_sigma(points, |_| true) else {
            return Self::from_bbox(exact);
        };

        // Pass 2: recompute, excluding points beyond 2σ from the raw mean.
        let lo = mean - 2.0 * sigma;
        let hi = mean + 2.0 * sigma;
        let Some((mean, sigma)) =
            mean_and_sigma(points, |d| d.cmpge(lo).all() && d.cmple(hi).all())
        else {
            return Self::from_bbox(exact);
        };

        let region_of_interest = macaw::BoundingBox::from_min_max(
            (mean - 2.0 * sigma).as_vec3(),
            (mean + 2.0 * sigma).as_vec3(),
        );

        Self {
            exact,
            region_of_interest,
        }
    }

    /// Transform both the bounding box and the region of interest.
    #[inline]
    pub fn transform_affine3(&self, transform: &glam::Affine3A) -> Self {
        Self {
            exact: self.exact.transform_affine3(transform),
            region_of_interest: self.region_of_interest.transform_affine3(transform),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    #[test]
    fn robust_bounds_excludes_outlier_from_region_of_interest() {
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
        let points = std::iter::chain(cluster_core.iter().copied(), std::iter::once(outlier))
            .collect::<Vec<_>>();

        let bounds = RobustBounds::from_points(&points);

        // The exact bbox must contain the outlier.
        assert!(
            bounds.exact.contains(outlier),
            "exact bounding box must contain outlier: {:?}",
            bounds.exact,
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

    #[test]
    fn robust_bounds_region_of_interest_can_exceed_exact_bounds() {
        // The eight corners of the unit box: mean 0.5 and σ 0.5 on each axis,
        // so `mean ± 2σ` spans [-0.5, 1.5] — twice the exact extent.
        let points = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(1.0, 1.0, 1.0),
        ];

        let bounds = RobustBounds::from_points(&points);

        assert_eq!(
            bounds.exact,
            macaw::BoundingBox::from_min_max(Vec3::ZERO, Vec3::ONE)
        );
        assert_eq!(
            bounds.region_of_interest,
            macaw::BoundingBox::from_min_max(Vec3::splat(-0.5), Vec3::splat(1.5)),
            "the region of interest may be larger than the exact bounds",
        );
    }
}
