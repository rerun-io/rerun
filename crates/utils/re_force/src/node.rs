use std::hash::Hash;

// TODO(grtlr): Control memory layout.

#[derive(Debug)]
pub(crate) struct Node<Ix: Hash + Eq> {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub ix: Ix,
    // The following fields signal that a node is fixed in a certain direction.
    // TODO(grtlr): Move this to a separate `Vec` in the simulation to improve the memory layout.
    pub fx: Option<f32>,
    pub fy: Option<f32>,
}

impl<Ix: Hash + Eq> Node<Ix> {
    pub fn new(ix: Ix, x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            fx: None,
            fy: None,
            ix,
        }
    }

    #[inline(always)]
    pub fn with_fixed_x(mut self) -> Self {
        self.fx = Some(self.x);
        self
    }

    #[inline(always)]
    pub fn with_fixed_y(mut self) -> Self {
        self.fx = Some(self.x);
        self
    }

    /// Applies the velocity to the vectors, while respecting fixed positions.
    pub(crate) fn apply_velocities(&mut self, velocity_decay: f32) {
        if let Some(fx) = self.fx {
            self.x = fx;
            self.vx = 0.0;
        } else {
            self.x += self.vx;
            self.vx *= velocity_decay;
        }

        if let Some(fy) = self.fy {
            self.y = fy;
            self.vy = 0.0;
        } else {
            self.y += self.vy;
            self.vy *= velocity_decay;
        }
    }
}

impl<Ix: Hash + Eq> From<Node<Ix>> for [f32; 2] {
    fn from(p: Node<Ix>) -> Self {
        [p.x, p.y]
    }
}

impl<Ix: Hash + Eq> From<(Ix, [f32; 2])> for Node<Ix> {
    fn from((ix, p): (Ix, [f32; 2])) -> Self {
        Self::new(ix, p[0], p[1])
    }
}
