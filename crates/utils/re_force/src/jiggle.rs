use rand::Rng;

pub fn jiggle<R: Rng>(rng: &mut R) -> f32 {
    (rng.gen::<f32>() - 0.5) * 1e-6
}
