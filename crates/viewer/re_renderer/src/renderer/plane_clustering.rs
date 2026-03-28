//! Helpers for grouping coplanar [`TexturedRect`]s into overlap clusters.

use crate::OutlineMaskPreference;

use super::rectangles::TexturedRect;

/// Per-rectangle clustering result.
#[derive(Clone, Copy)]
pub(crate) struct RectangleClusterInfo {
    /// Shared world-space position for the cluster.
    pub sorting_position: glam::Vec3A,

    /// Whether this rectangle belongs to a multi-rectangle overlap cluster
    /// and therefore needs to be forced into the transparent draw phase to avoid z-fighting.
    pub force_transparent: bool,

    /// Outline mask propagated to all members of the cluster.
    pub outline_mask: OutlineMaskPreference,
}

/// Identifies a plane bucket for grouping rectangles that are approximately coplanar.
///
/// Distance and normal are quantized for efficiency, with the normal using octahedral encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PlanarBucketId {
    quantized_normal: [i16; 2],
    quantized_distance: i32,
}

#[derive(Clone)]
struct BucketedRectangle {
    index: usize,
    center: glam::Vec3A,
    projected_rect: OrientedRect2D,
    outline_mask: OutlineMaskPreference,
}

#[derive(Clone, Copy)]
struct OrientedRect2D {
    center: glam::Vec2,
    axes: [glam::Vec2; 2],
    half_extents: [f32; 2],
}

/// Groups rectangles into coplanar overlap clusters.
///
/// Single rectangles keep their own center and outline mask.
/// Connected overlap components with more than one rectangle get a shared sort position,
/// a merged outline mask, and are forced into the transparent draw phase.
pub(crate) fn cluster_rectangles(rectangles: &[TexturedRect]) -> Vec<RectangleClusterInfo> {
    re_tracing::profile_function!();

    // Start with each rectangle using its own center for sorting. Only rectangles that later turn
    // out to belong to an overlapping coplanar cluster get rewritten to share one cluster center.
    let mut cluster_infos = rectangles
        .iter()
        .map(|rectangle| RectangleClusterInfo {
            sorting_position: rectangle.center(),
            force_transparent: false,
            outline_mask: rectangle.options.outline_mask,
        })
        .collect::<Vec<_>>();

    for bucket_rectangles in bucket_rectangles_by_plane(rectangles).into_values() {
        for cluster in overlapping_clusters(&bucket_rectangles) {
            if cluster.len() <= 1 {
                continue;
            }

            // All rectangles in one connected overlap component share a single distance sort key
            // so the renderer compares them as one depth-sorted group. Their individual ordering
            // inside the group is still decided by the secondary sort key.
            let cluster_center =
                cluster.iter().map(|rect| rect.center).sum::<glam::Vec3A>() / cluster.len() as f32;

            // We have to merge the outline mask within a cluster to avoid z-fighting,
            // as the outline is a separate pass that always writes to depth buffer.
            // This would lead to flickering outlines if two rectangles with different outline masks overlap and occlude each other.
            //
            // Tradeoff: The drawback is that we might end up with a larger outline than strictly necessary,
            // but this is a lot better than flickering.
            let cluster_outline_mask = merged_outline_mask(&cluster);

            for rectangle in cluster {
                cluster_infos[rectangle.index] = RectangleClusterInfo {
                    sorting_position: cluster_center,
                    force_transparent: true,
                    outline_mask: cluster_outline_mask,
                };
            }
        }
    }

    cluster_infos
}

fn bucket_rectangles_by_plane(
    rectangles: &[TexturedRect],
) -> std::collections::HashMap<PlanarBucketId, Vec<BucketedRectangle>> {
    let mut buckets: std::collections::HashMap<PlanarBucketId, Vec<BucketedRectangle>> =
        std::collections::HashMap::default();

    for (index, rectangle) in rectangles.iter().enumerate() {
        let (Some(bucket_id), Some(projected_rect)) =
            (planar_bucket_id(rectangle), projected_rect(rectangle))
        else {
            // Degenerate rectangles cannot contribute to a planar overlap cluster.
            continue;
        };

        // Only rectangles that quantize to the same plane bucket can ever interact as a layered
        // group, so we keep the more expensive overlap test local to each bucket.
        buckets
            .entry(bucket_id)
            .or_default()
            .push(BucketedRectangle {
                index,
                center: rectangle.center(),
                projected_rect,
                outline_mask: rectangle.options.outline_mask,
            });
    }

    buckets
}

/// Computes connected overlap components within a single plane bucket.
///
/// Two rectangles are adjacent if their projected 2D rectangles overlap. Each connected
/// component becomes one cluster.
fn overlapping_clusters(bucket_rectangles: &[BucketedRectangle]) -> Vec<Vec<&BucketedRectangle>> {
    // Treat rectangles in one plane bucket as nodes in an undirected graph where edges denote
    // geometric overlap. Each connected component becomes one cluster that shares a single
    // distance sort key.
    let mut visited = vec![false; bucket_rectangles.len()];
    let mut clusters = Vec::new();

    for start_idx in 0..bucket_rectangles.len() {
        if visited[start_idx] {
            continue;
        }

        // We want to find all rectangles that belong to the same connected component.
        // For this, we need to check the starting rectangle first.
        // If it has an overlap with any other rectangle, we add that to this stack to also check its neighbors later.
        let mut remaining_unchecked_cluster_members = vec![start_idx];
        visited[start_idx] = true;
        let mut current_cluster = Vec::new();

        while let Some(current_idx) = remaining_unchecked_cluster_members.pop() {
            let current = &bucket_rectangles[current_idx];
            // This rectangle hasn't been visited yet, so it belongs to a new cluster.
            current_cluster.push(current);

            // Walk the overlap graph with a simple DFS: if two rectangles overlap in the same
            // plane bucket, they belong to the same connected component and therefore share one
            // distance sort key.
            for (next_idx, next) in bucket_rectangles.iter().enumerate() {
                if visited[next_idx] {
                    // Other visited rectangles belong to another cluster, so skip.
                    continue;
                }

                if rectangles_overlap(&current.projected_rect, &next.projected_rect) {
                    visited[next_idx] = true;
                    // This rectangle overlaps, so we also need to transitively visit its neighbors.
                    // Push it to the stack to visit its neighbors later.
                    remaining_unchecked_cluster_members.push(next_idx);
                }
            }
        }

        clusters.push(current_cluster);
    }

    clusters
}

/// Merges outline masks across one overlap cluster.
fn merged_outline_mask(cluster: &[&BucketedRectangle]) -> OutlineMaskPreference {
    cluster
        .iter()
        .fold(OutlineMaskPreference::NONE, |merged, rectangle| {
            merged.with_fallback_to(rectangle.outline_mask)
        })
}

/// Computes the quantized plane bucket for a rectangle.
fn planar_bucket_id(rectangle: &TexturedRect) -> Option<PlanarBucketId> {
    let normal = canonical_plane_normal(rectangle.extent_u.cross(rectangle.extent_v))?;
    let distance = normal.dot(rectangle.top_left_corner_position);

    Some(PlanarBucketId {
        quantized_normal: quantize_normal_octahedral(normal),
        // TODO(michael): This step size is heuristic and might need revisiting for large scale differences.
        quantized_distance: (distance / 1.0e-3).round() as i32,
    })
}

/// Projects a world-space rectangle into an arbitrary orthonormal basis of its plane.
fn projected_rect(rectangle: &TexturedRect) -> Option<OrientedRect2D> {
    let normal = canonical_plane_normal(rectangle.extent_u.cross(rectangle.extent_v))?;
    // Build an arbitrary orthonormal basis for the plane so overlap checks can happen in 2D.
    // Once rectangles are known to be coplanar, their relative overlap is independent of which
    // in-plane basis we choose.
    let basis_u = if normal.z.abs() < 0.999 {
        normal.cross(glam::Vec3::Z).try_normalize()?
    } else {
        normal.cross(glam::Vec3::X).try_normalize()?
    };
    let basis_v = normal.cross(basis_u);
    let project = |point: glam::Vec3| glam::vec2(point.dot(basis_u), point.dot(basis_v));

    let extent_u_2d = project(rectangle.extent_u);
    let extent_v_2d = project(rectangle.extent_v);
    let half_u = extent_u_2d.length() * 0.5;
    let half_v = extent_v_2d.length() * 0.5;
    // TODO(michael): this could potentially use a threshold for near degenerate rectangles.
    // These are however anyway sorted out later by the planar bucketing.
    if half_u == 0.0 || half_v == 0.0 {
        return None;
    }

    // Project the rectangle into that common 2D basis. Because the source geometry is always a
    // right-angled rectangle, this gives us an oriented rectangle representation directly.
    Some(OrientedRect2D {
        center: project(
            rectangle.top_left_corner_position
                + rectangle.extent_u * 0.5
                + rectangle.extent_v * 0.5,
        ),
        axes: [extent_u_2d / (half_u * 2.0), extent_v_2d / (half_v * 2.0)],
        half_extents: [half_u, half_v],
    })
}

/// Canonicalizes a plane normal so equivalent planes with flipped normals map to the same bucket.
fn canonical_plane_normal(normal: glam::Vec3) -> Option<glam::Vec3> {
    let normal = normal.try_normalize()?;
    let canonical_sign = if normal.z != 0.0 {
        normal.z.signum()
    } else if normal.y != 0.0 {
        normal.y.signum()
    } else {
        normal.x.signum()
    };

    Some(if canonical_sign < 0.0 {
        -normal
    } else {
        normal
    })
}

/// Tests overlap between two projected oriented rectangles using the Separating Axis Theorem (SAT).
fn rectangles_overlap(a: &OrientedRect2D, b: &OrientedRect2D) -> bool {
    // For rectangles (orthogonal shape), SAT only needs the two local axes of each rectangle.
    overlap_on_axis(a, b, a.axes[0])
        && overlap_on_axis(a, b, a.axes[1])
        && overlap_on_axis(a, b, b.axes[0])
        && overlap_on_axis(a, b, b.axes[1])
}

/// Tests overlap along one separating axis.
fn overlap_on_axis(a: &OrientedRect2D, b: &OrientedRect2D, axis: glam::Vec2) -> bool {
    let center_distance = (b.center - a.center).dot(axis).abs();
    let a_radius = project_rect_radius(a, axis);
    let b_radius = project_rect_radius(b, axis);
    center_distance <= a_radius + b_radius
}

/// Returns the projection radius of an oriented rectangle on one axis.
fn project_rect_radius(rect: &OrientedRect2D, axis: glam::Vec2) -> f32 {
    rect.half_extents[0] * rect.axes[0].dot(axis).abs()
        + rect.half_extents[1] * rect.axes[1].dot(axis).abs()
}

/// Quantizes a unit normal with octahedral encoding.
fn quantize_normal_octahedral(normal: glam::Vec3) -> [i16; 2] {
    let projected = normal / (normal.x.abs() + normal.y.abs() + normal.z.abs());
    let encoded = if projected.z < 0.0 {
        let folded = glam::vec2(projected.y, projected.x);
        (glam::Vec2::ONE - folded.abs()) * glam::vec2(projected.x, projected.y).signum()
    } else {
        glam::vec2(projected.x, projected.y)
    };

    [
        quantize_signed_unit_to_i16(encoded.x),
        quantize_signed_unit_to_i16(encoded.y),
    ]
}

/// Quantizes a signed unit-range float to `i16`.
fn quantize_signed_unit_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}
