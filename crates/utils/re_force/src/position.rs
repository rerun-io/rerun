use crate::node::Node;
use std::hash::Hash;

#[derive(Clone, Debug)]
pub struct PositionX {
    strength: f32,
    x: f32,
}

impl Default for PositionX {
    fn default() -> Self {
        Self {
            strength: 0.1,
            x: 0.0,
        }
    }
}

impl PositionX {
    pub fn force<Ix: Hash + Eq>(&mut self, alpha: f32, nodes: &mut [Node<Ix>]) {
        let strengths = std::iter::repeat(self.strength);

        for (node, si) in nodes.iter_mut().zip(strengths) {
            let d = self.x - node.x;
            node.vx += d * si * alpha;
        }
    }
}

#[derive(Clone, Debug)]
pub struct PositionY {
    strength: f32,
    y: f32,
}

impl Default for PositionY {
    fn default() -> Self {
        Self {
            strength: 0.1,
            y: 0.0,
        }
    }
}

impl PositionY {
    pub fn force<Ix: Hash + Eq>(&mut self, alpha: f32, nodes: &mut [Node<Ix>]) {
        let strengths = std::iter::repeat(self.strength);

        for (node, si) in nodes.iter_mut().zip(strengths) {
            let d = self.y - node.y;
            node.vy += d * si * alpha;
        }
    }
}
