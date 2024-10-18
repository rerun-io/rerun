use std::{collections::HashMap, hash::Hash};

use rand::thread_rng;

use crate::{
    func::{constant, DistanceFn, StrengthFn},
    jiggle::jiggle,
    node::Node,
};

pub struct LinkBuilder<Ix: Hash + Eq + Clone> {
    links: Vec<(Ix, Ix)>,
    distance: DistanceFn<Ix>,
    strength: StrengthFn<Ix>,
    strengths: Vec<f32>,
    iterations: usize,
}

impl<Ix: Hash + Eq + Clone> LinkBuilder<Ix> {
    pub fn new(links: Vec<(Ix, Ix)>) -> Self {
        Self {
            links,
            distance: constant(20.0),
            strength: constant(1.0),
            strengths: Vec::new(),
            iterations: 1,
        }
    }

    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn initialize(&self, nodes: &[Node<Ix>]) -> Link<Ix> {
        Link::new(nodes, self.links.clone())
    }

    // fn initialize_strengths(&self, nodes: &[Node<Ix>]) -> Vec<f32> {
    //     nodes
    //         .iter()
    //         .enumerate()
    //         .map(|(i, node)| self.strength.0(&node.ix, i))
    //         .collect()
    // }
}

#[derive(Debug)]
pub struct Link<Ix: Hash + Eq + Clone> {
    links: Vec<(Ix, Ix)>,
    node_by_id: HashMap<Ix, usize>,

    // TODO(grtlr): These two are arrays in d3.
    count: HashMap<Ix, usize>,
    bias: HashMap<(Ix, Ix), f32>,

    // TODO(grtlr): In `d3`, the following fields are computed using variable functions. For now we just use defaults.
    strengths: Vec<f32>,

    distances: Vec<f32>,

    iterations: usize,
}

impl<Ix: Hash + Eq + Clone> Link<Ix> {
    pub fn new(nodes: &[Node<Ix>], links: Vec<(Ix, Ix)>) -> Self {
        let node_by_id = nodes
            .iter()
            .enumerate()
            .map(|(arr_ix, node)| (node.ix.clone(), arr_ix))
            .collect::<HashMap<_, _>>();

        let mut count = HashMap::new();
        for link in &links {
            *count.entry(link.0.clone()).or_insert(0) += 1;
            *count.entry(link.1.clone()).or_insert(0) += 1;
        }

        let bias = links
            .iter()
            .cloned()
            .map(|link| {
                (
                    link.clone(),
                    count[&link.0] as f32 / (count[&link.0] + count[&link.1]) as f32,
                )
            })
            .collect();

        // TODO(grtlr): For now we only implement the `defaultStrength` function.
        let strengths = links
            .iter()
            .map(|link| 1.0 / usize::min(count[&link.0], count[&link.1]) as f32)
            .collect();

        // TODO(grtlr): Move this to the builder
        let mut distance: DistanceFn<Ix> = constant(30.0);
        let distances = links
            .iter()
            .enumerate()
            .map(|(i, edge)| distance.0(edge, i))
            .collect();

        Self {
            links,
            node_by_id,
            count,
            bias,
            strengths,
            distances,
            iterations: 1,
        }
    }

    pub fn force(&mut self, alpha: f32, nodes: &mut [Node<Ix>]) {
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
