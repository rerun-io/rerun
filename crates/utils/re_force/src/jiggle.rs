use rand::Rng;

// TODO(grtlr): refactor this to be optional

pub fn jiggle<R: Rng>(rng: &mut R) -> f32 {
    (rng.gen::<f32>() - 0.5) * 1e-6
}
