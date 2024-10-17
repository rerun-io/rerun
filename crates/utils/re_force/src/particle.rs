use emath::{Pos2, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    // TODO: hide implementation details
    pub pos: Pos2,
    pub(crate) vel: Vec2,
}

impl Particle {
    pub fn new<'a>(pos: impl Into<[f32; 2]>) -> Self {
        let pos: [f32; 2] = pos.into();
        Particle {
            pos: pos.into(),
            vel: Vec2::ZERO,
        }
    }
}

impl From<Particle> for [f32; 2] {
    fn from(p: Particle) -> Self {
        p.pos.into()
    }
}
