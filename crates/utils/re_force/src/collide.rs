use emath::{Pos2, Rect, Vec2};
use rand::thread_rng;

use crate::{jiggle::jiggle, particle::Particle};

pub struct Collide {
    radii: Option<Vec<f32>>,
    strength: f32,
    iterations: usize,
}

impl Default for Collide {
    fn default() -> Self {
        Collide {
            radii: Default::default(),
            strength: 1.0,
            iterations: 1,
        }
    }
}

impl Collide {
    // TODO: speed up using quadtree
    pub fn force(&mut self, particles: &mut [Particle]) {
        // TODO: make this configurable
        let radii: Vec<_> = (0..particles.len()).map(|_| 10.0).collect();

        debug_assert!(radii.len() == particles.len());

        for _ in 0..self.iterations {
            for s in 0..particles.len() {
                let (left, right) = particles.split_at_mut(s);

                for (i, node) in left.iter_mut().enumerate() {
                    let ri = radii[i];
                    let ri2 = ri * ri;
                    let ni = node.pos + node.vel;

                    for (j, other) in right.iter_mut().enumerate() {
                        let rj = radii[s + j];

                        let r = ri + rj;
                        let mut d = ni - other.pos - other.vel;
                        let mut l = d.x * d.x + d.y * d.y;
                        if l < r * r {
                            // We need to resolve points that coincide.
                            if d.x == 0.0 {
                                d.x = jiggle(&mut thread_rng());
                                l += d.x * d.x;
                            }
                            if d.y == 0.0 {
                                d.y = jiggle(&mut thread_rng());
                                l += d.y * d.y;
                            }

                            l = l.sqrt();
                            l = (r - l) / l * self.strength;
                            d *= l;
                            let rj2 = rj * rj;
                            let frac = rj2 / (ri2 + rj2);
                            node.vel += d * frac;
                            other.vel -= d * (1.0 - frac);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn resolve_all_coincide() {
        let mut particles = std::iter::repeat(Particle {
            pos: Pos2::ZERO,
            vel: Vec2::ZERO,
        })
        .take(5)
        .collect::<Vec<_>>();

        let mut collide = Collide::default();

        collide.force(&mut particles);

        assert_ne!(particles[0].vel, Vec2::ZERO);
        assert_ne!(particles[1].vel, Vec2::ZERO);
        assert_ne!(particles[2].vel, Vec2::ZERO);
        assert_ne!(particles[3].vel, Vec2::ZERO);
        assert_ne!(particles[4].vel, Vec2::ZERO);
    }
}
