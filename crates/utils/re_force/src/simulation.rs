use std::collections::HashMap;
use std::hash::Hash;

use crate::{
    collide::Collide,
    lcg::LCG,
    many_body::{ManyBody, ManyBodyBuilder},
    node::Node,
    position::{PositionX, PositionY},
    LinkBuilder,
};

enum Force<Ix: Hash + Eq + Clone> {
    Collide(Collide),
    PositionX(PositionX),
    PositionY(PositionY),
    Link(LinkBuilder<Ix>),
    ManyBody(ManyBody),
}

#[derive(Debug)]
pub struct SimulationBuilder {
    alpha: f32,
    alpha_min: f32,
    alpha_decay: f32,
    alpha_target: f32,
    velocity_decay: f32,
    _random: LCG,
}

impl Default for SimulationBuilder {
    fn default() -> Self {
        let alpha_min = 0.001;
        Self {
            alpha: 1.0,
            alpha_min,
            alpha_decay: 1.0 - alpha_min.powf(1.0 / 300.0),
            alpha_target: 0.0,
            velocity_decay: 0.6,
            _random: LCG::default(),
        }
    }
}

impl SimulationBuilder {
    // TODO(grtlr): build with fixed positions!

    #[inline(always)]
    pub fn build<Ix, P>(&self, nodes: impl IntoIterator<Item = (Ix, P)>) -> Simulation<Ix>
    where
        P: Into<[f32; 2]>,
        Ix: Hash + Eq + Clone,
    {
        let nodes = nodes.into_iter().map(|(ix, p)| {
            let p = p.into();
            Node::new(ix, p[0], p[1])
        });

        Simulation {
            alpha: self.alpha,
            alpha_min: self.alpha_min,
            alpha_decay: self.alpha_decay,
            alpha_target: self.alpha_target,
            velocity_decay: self.velocity_decay,
            nodes: nodes.collect(),
            _random: self._random.clone(),
            forces: Default::default(),
        }
    }
}

pub struct Simulation<Ix>
where
    Ix: Hash + Eq + Clone,
{
    alpha: f32,
    alpha_min: f32,
    alpha_decay: f32,
    alpha_target: f32,
    velocity_decay: f32,
    _random: LCG,
    forces: HashMap<String, Force<Ix>>,
    nodes: Vec<Node<Ix>>,
}

// TODO(grtlr): Could simulation be an iterator?
impl<Ix: Hash + Eq + Clone> Simulation<Ix> {
    pub fn step(&mut self) {
        while self.alpha < self.alpha_min {
            self.tick(1);
        }
    }

    pub fn tick(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.alpha += (self.alpha_target - self.alpha) * self.alpha_decay;

            for force in &mut self.forces.values_mut() {
                match force {
                    Force::Collide(c) => c.force(&mut self.nodes),
                    Force::PositionX(p) => p.force(self.alpha, &mut self.nodes),
                    Force::PositionY(p) => p.force(self.alpha, &mut self.nodes),
                    Force::Link(l) => {
                        // TODO(grtlr): don't rebuild the forces on every run, separate the build and run steps instead.
                        l.initialize(&self.nodes).force(self.alpha, &mut self.nodes);
                    }
                    Force::ManyBody(m) => {
                        // TODO(grtlr): don't rebuild the forces on every run, separate the build and run steps instead.
                        m.force(self.alpha, &mut self.nodes);
                    }
                }
            }

            for n in &mut self.nodes {
                n.apply_velocities(self.velocity_decay);
            }
        }
    }

    pub fn positions<'a>(&'a self) -> impl Iterator<Item = (&'a Ix, [f32; 2])> {
        self.nodes
            .iter()
            .map(move |n: &'a Node<Ix>| (&n.ix, [n.x, n.y]))
    }

    #[inline(always)]
    pub fn add_force_collide(mut self, name: String, force: Collide) -> Self {
        self.forces.insert(name, Force::Collide(force));
        self
    }

    #[inline(always)]
    pub fn add_force_x(mut self, name: String, force: PositionX) -> Self {
        self.forces.insert(name, Force::PositionX(force));
        self
    }

    #[inline(always)]
    pub fn add_force_y(mut self, name: String, force: PositionY) -> Self {
        self.forces.insert(name, Force::PositionY(force));
        self
    }

    #[inline(always)]
    pub fn add_force_link(mut self, name: String, force: LinkBuilder<Ix>) -> Self {
        self.forces.insert(name, Force::Link(force));
        self
    }

    #[inline(always)]
    pub fn add_force_many_body(mut self, name: String, builder: ManyBodyBuilder<Ix>) -> Self {
        let force = builder.initialize(&self.nodes);
        self.forces.insert(name, Force::ManyBody(force));
        self
    }
}

impl<Ix: Hash + Eq + Clone> From<Simulation<Ix>> for SimulationBuilder {
    fn from(value: Simulation<Ix>) -> Self {
        Self {
            alpha: value.alpha,
            alpha_min: value.alpha_min,
            alpha_decay: value.alpha_decay,
            alpha_target: value.alpha_target,
            velocity_decay: value.velocity_decay,
            _random: value._random,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_tick() {
        let particles = [[0.0f32, 1.0], [0.0, -1.0]];

        let mut simulation = Simulation::new(particles);
        let particles = simulation.tick(1000000);

        assert_ne!(particles[0].pos, particles[1].pos);
        assert_eq!(particles[0].pos.x, 0.0);
        assert_eq!(particles[1].pos.x, 0.0);
    }
}
