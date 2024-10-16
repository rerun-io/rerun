use crate::particle::Particle;

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
    pub fn force(&mut self, alpha: f32, particles: &mut [Particle]) {
        let strengths = std::iter::repeat(self.strength);

        for (particle, si) in particles.iter_mut().zip(strengths) {
            let d = self.x - particle.pos.x;
            particle.vel.x += d * si * alpha;
        }
    }
}

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
    pub fn force(&mut self, alpha: f32, particles: &mut [Particle]) {
        let strengths = std::iter::repeat(self.strength);

        for (particle, si) in particles.iter_mut().zip(strengths) {
            let d = self.y - particle.pos.y;
            particle.vel.y += d * si * alpha;
        }
    }
}
