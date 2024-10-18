use std::{collections::HashMap, hash::Hash};

use rand::thread_rng;

use crate::{
    func::{constant, LinkFn},
    jiggle::jiggle,
    node::Node,
};

pub struct LinkBuilder<Ix: Hash + Eq + Clone> {
    links: Vec<(Ix, Ix)>,
    strength: Option<LinkFn<Ix>>,
    distance: LinkFn<Ix>,
    iterations: usize,
}

impl<Ix: Hash + Eq + Clone> LinkBuilder<Ix> {
    pub fn new(links: Vec<(Ix, Ix)>) -> Self {
        Self {
            links,
            // TODO(grtlr): Change this back
            distance: constant(20.0),
            // TODO(grtlr): Change this back to `None` to match `d3`.
            strength: Some(constant(1.0)),

            // TODO(grtlr): Return this back to 1
            iterations: 20,
        }
    }

    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn initialize(mut self, nodes: &[Node<Ix>]) -> Option<Link<Ix>> {
        if nodes.is_empty() {
            return None;
        }

        let node_by_id = nodes
            .iter()
            .enumerate()
            .map(|(arr_ix, node)| (node.ix.clone(), arr_ix))
            .collect::<HashMap<_, _>>();

        // TODO(grtlr): This is in array d3.
        let mut count = HashMap::new();
        for link in &self.links {
            *count.entry(link.0.clone()).or_insert(0) += 1;
            *count.entry(link.1.clone()).or_insert(0) += 1;
        }

        let bias = self
            .links
            .iter()
            .cloned()
            .map(|link| {
                (
                    link.clone(),
                    count[&link.0] as f32 / (count[&link.0] + count[&link.1]) as f32,
                )
            })
            .collect();

        let strengths = self
            .links
            .iter()
            .enumerate()
            .map(|(i, link)| {
                if let Some(strength) = &mut self.strength {
                    strength.0(link, i)
                } else {
                    1.0 / usize::min(count[&link.0], count[&link.1]) as f32
                }
            })
            .collect();

        let distances = self
            .links
            .iter()
            .enumerate()
            .map(|link| self.distance.apply(link))
            .collect();

        Some(Link {
            links: self.links,
            node_by_id,
            bias,
            strengths,
            distances,
            iterations: self.iterations,
        })
    }
}

#[derive(Debug)]
pub struct Link<Ix: Hash + Eq + Clone> {
    links: Vec<(Ix, Ix)>,
    node_by_id: HashMap<Ix, usize>,

    // TODO(grtlr): This is in array d3.
    bias: HashMap<(Ix, Ix), f32>,

    // TODO(grtlr): In `d3`, the following fields are computed using variable functions. For now we just use defaults.
    strengths: Vec<f32>,

    distances: Vec<f32>,

    iterations: usize,
}

impl<Ix: Hash + Eq + Clone> Link<Ix> {
    pub fn force(&mut self, alpha: f32, nodes: &mut [Node<Ix>]) {
        for _ in 0..self.iterations {
            for (i, link) in self.links.iter().enumerate() {
                let (source, target) = link;
                let (source, target) = (
                    &nodes[self.node_by_id[source]],
                    &nodes[self.node_by_id[target]],
                );

                let mut x = target.x + target.vx - source.x - source.vx;
                if x == 0.0 {
                    x = jiggle(&mut thread_rng());
                }
                let mut y = target.y + target.vy - source.y - source.vy;
                if y == 0.0 {
                    y = jiggle(&mut thread_rng());
                }
                let l = x.hypot(y);
                let l = (l - self.distances[i]) / l * self.strengths[i] * alpha;

                let bx = self.bias[&(source.ix.clone(), target.ix.clone())];
                let by = 1.0 - bx;

                nodes[self.node_by_id[&link.0]].vx += x * l * bx;
                nodes[self.node_by_id[&link.0]].vy += y * l * bx;
                nodes[self.node_by_id[&link.1]].vx -= x * l * by;
                nodes[self.node_by_id[&link.1]].vy -= y * l * by;
            }
        }
    }
}
