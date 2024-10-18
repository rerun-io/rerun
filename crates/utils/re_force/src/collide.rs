use rand::thread_rng;
use std::hash::Hash;

use crate::{jiggle::jiggle, node::Node};

#[derive(Clone, Debug)]
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
    pub fn force<Ix: Hash + Eq + Clone>(&mut self, nodes: &mut [Node<Ix>]) {
        // TODO: make this configurable
        let radii: Vec<_> = (0..nodes.len()).map(|_| 10.0).collect();

        debug_assert!(radii.len() == nodes.len());

        for _ in 0..self.iterations {
            for s in 0..nodes.len() {
                let (left, right) = nodes.split_at_mut(s);

                for (i, node) in left.iter_mut().enumerate() {
                    let ri = radii[i];
                    let ri2 = ri * ri;
                    let xi = node.x + node.vx;
                    let yi = node.y + node.vy;

                    for (j, data) in right.iter_mut().enumerate() {
                        let rj = radii[s + j];

                        let r = ri + rj;
                        let mut x = xi - data.x - data.vx;
                        let mut y = yi - data.y - data.vx;
                        let mut l = x * x + y * y;
                        if l < r * r {
                            // We need to resolve points that coincide.
                            if x == 0.0 {
                                x = jiggle(&mut thread_rng());
                                l += x * x;
                            }
                            if y == 0.0 {
                                y = jiggle(&mut thread_rng());
                                l += y * y;
                            }

                            l = l.sqrt();
                            l = (r - l) / l * self.strength;
                            x *= l;
                            y *= l;
                            let rj2 = rj * rj;
                            let frac = rj2 / (ri2 + rj2);
                            node.vx += x * frac;
                            node.vy += y * frac;
                            data.vx -= x * (1.0 - frac);
                            data.vy -= y * (1.0 - frac);
                        }
                    }
                }
            }
        }
    }
}
