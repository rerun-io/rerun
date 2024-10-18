use std::hash::Hash;

#[repr(transparent)]
pub struct DistanceFn<Ix: Hash + Eq + Clone>(pub Box<dyn FnMut(&(Ix, Ix), usize) -> f32>);

impl<Ix: Hash + Eq + Clone> From<f32> for DistanceFn<Ix> {
    #[inline(always)]
    fn from(value: f32) -> Self {
        Self(Box::new(move |_, _| value))
    }
}

#[repr(transparent)]
pub struct StrengthFn<Ix: Hash + Eq + Clone>(pub Box<dyn FnMut(&Ix, usize) -> f32>);

impl<Ix: Hash + Eq + Clone> From<f32> for StrengthFn<Ix> {
    #[inline(always)]
    fn from(value: f32) -> Self {
        Self(Box::new(move |_, _| value))
    }
}

#[inline(always)]
pub fn constant<F>(value: f32) -> F
where
    F: From<f32>,
{
    value.into()
}
