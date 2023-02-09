use egui::{pos2, Pos2};

/// Positions circles on a horizontal line with some vertical scattering to avoid overlap.
pub struct BallScatterer {
    recent: [Pos2; Self::MEMORY_SIZE],
    cursor: usize,
}

impl Default for BallScatterer {
    fn default() -> Self {
        Self {
            recent: [Pos2::new(f32::INFINITY, f32::INFINITY); Self::MEMORY_SIZE],
            cursor: 0,
        }
    }
}

impl BallScatterer {
    const MEMORY_SIZE: usize = 8;

    pub fn add(&mut self, x: f32, r: f32, (min_y, max_y): (f32, f32)) -> Pos2 {
        let min_y = min_y + r; // some padding
        let max_y = max_y - r; // some padding

        let r2 = r * r * 3.0; // allow some overlap

        let center_y = 0.5 * (min_y + max_y);

        let y = if max_y <= min_y {
            center_y
        } else {
            let mut best_free_y = f32::INFINITY;
            let mut best_colliding_y = center_y;
            let mut best_colliding_d2 = 0.0;

            let step_size = 2.0; // unit: points

            for y_offset in 0..=((max_y - min_y) / step_size).round() as i32 {
                let y = min_y + step_size * y_offset as f32;
                let d2 = self.closest_dist_sq(&pos2(x, y));
                let intersects = d2 < r2;
                if intersects {
                    // pick least colliding
                    if d2 > best_colliding_d2 {
                        best_colliding_y = y;
                        best_colliding_d2 = d2;
                    }
                } else {
                    // pick closest to center
                    if (y - center_y).abs() < (best_free_y - center_y).abs() {
                        best_free_y = y;
                    }
                }
            }

            if best_free_y.is_finite() {
                best_free_y
            } else {
                best_colliding_y
            }
        };

        let pos = pos2(x, y);
        self.recent[self.cursor] = pos;
        self.cursor = (self.cursor + 1) % Self::MEMORY_SIZE;
        pos
    }

    fn closest_dist_sq(&self, pos: &Pos2) -> f32 {
        let mut d2 = f32::INFINITY;
        for recent in &self.recent {
            d2 = d2.min(recent.distance_sq(*pos));
        }
        d2
    }
}
