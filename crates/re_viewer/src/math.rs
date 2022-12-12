pub use egui::emath::{remap, remap_clamp, Rect};

pub use macaw::BoundingBox;

pub fn line_segment_distance_sq_to_point_2d([a, b]: [glam::Vec2; 2], p: glam::Vec2) -> f32 {
    let l2 = a.distance_squared(b);
    if l2 == 0.0 {
        a.distance_squared(p)
    } else {
        let t = ((p - a).dot(b - a) / l2).clamp(0.0, 1.0);
        let projection = a + t * (b - a);
        p.distance_squared(projection)
    }
}

pub fn line_segment_distance_sq_to_point_3d([a, b]: [glam::Vec3; 2], p: glam::Vec3) -> f32 {
    let l2 = a.distance_squared(b);
    if l2 == 0.0 {
        a.distance_squared(p)
    } else {
        let t = ((p - a).dot(b - a) / l2).clamp(0.0, 1.0);
        let projection = a + t * (b - a);
        p.distance_squared(projection)
    }
}

pub fn line_segment_distance_to_point_3d([a, b]: [glam::Vec3; 2], p: glam::Vec3) -> f32 {
    line_segment_distance_sq_to_point_3d([a, b], p).sqrt()
}

/// Returns the distance the ray traveled of the first intersection or `f32::INFINITY` on miss.
pub fn ray_bbox_intersect(ray: &macaw::Ray3, bbox: &macaw::BoundingBox) -> f32 {
    // from https://gamedev.stackexchange.com/a/18459

    let t1 = (bbox.min.x - ray.origin.x) / ray.dir.x;
    let t2 = (bbox.max.x - ray.origin.x) / ray.dir.x;
    let t3 = (bbox.min.y - ray.origin.y) / ray.dir.y;
    let t4 = (bbox.max.y - ray.origin.y) / ray.dir.y;
    let t5 = (bbox.min.z - ray.origin.z) / ray.dir.z;
    let t6 = (bbox.max.z - ray.origin.z) / ray.dir.z;

    let tmin = max(max(min(t1, t2), min(t3, t4)), min(t5, t6));
    let tmax = min(min(max(t1, t2), max(t3, t4)), max(t5, t6));

    if tmax < 0.0 || tmax < tmin {
        f32::INFINITY
    } else {
        tmin
    }
}

fn min(a: f32, b: f32) -> f32 {
    a.min(b)
}
fn max(a: f32, b: f32) -> f32 {
    a.max(b)
}

pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t) * (1. - t)
}
