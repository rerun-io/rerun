use egui::emath::*;

pub fn line_segment_distance_sq_to_point([a, b]: [Pos2; 2], p: Pos2) -> f32 {
    let l2 = a.distance_sq(b);
    if l2 == 0.0 {
        a.distance_sq(p)
    } else {
        let t = ((p - a).dot(b - a) / l2).clamp(0.0, 1.0);
        let projection = a + t * (b - a);
        p.distance_sq(projection)
    }
}
