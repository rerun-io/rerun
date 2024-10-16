use emath::Vec2;

use crate::{
    collide::Collide,
    lcg::LCG,
    particle::Particle,
    position::{PositionX, PositionY},
};

enum Force {
    Collide(Collide),
    PositionX(PositionX),
    PositionY(PositionY),
}

pub struct Simulation {
    alpha: f32,
    alpha_min: f32,
    alpha_decay: f32,
    alpha_target: f32,
    velocity_decay: f32,
    random: LCG,
    forces: Vec<Force>,
    particles: Vec<Particle>,
}

impl Simulation {
    pub fn new<'a>(particles: impl IntoIterator<Item = [f32; 2]>) -> Self {
        let alpha_min = 0.001;
        Simulation {
            alpha: 1.0,
            alpha_min,
            alpha_decay: 1.0 - alpha_min.powf(1.0 / 300.0),
            alpha_target: 0.0,
            velocity_decay: 0.6,
            random: LCG::default(),
            forces: vec![
                Force::Collide(Collide::default()),
                Force::PositionX(PositionX::default()),
                Force::PositionY(PositionY::default()),
            ],
            particles: particles.into_iter().map(Particle::new).collect(),
        }
    }
}

impl Simulation {
    pub fn step(&mut self) -> &[Particle] {
        while self.alpha < self.alpha_min {
            self.tick(1);
        }
        &self.particles
    }

    pub fn tick(&mut self, iterations: usize) -> &[Particle] {
        for _ in 0..iterations {
            self.alpha += (self.alpha_target - self.alpha) * self.alpha_decay;

            for force in &mut self.forces {
                match force {
                    Force::Collide(c) => c.force(&mut self.particles),
                    Force::PositionX(p) => p.force(self.alpha, &mut self.particles),
                    Force::PositionY(p) => p.force(self.alpha, &mut self.particles),
                }
            }

            for particle in &mut self.particles {
                particle.vel *= self.velocity_decay;
                particle.pos += particle.vel;
                particle.vel = Vec2::ZERO;
            }
        }

        &self.particles
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
