use std::hash::Hash;

#[repr(transparent)]
pub struct LinkFn<Ix: Hash + Eq + Clone>(pub Box<dyn FnMut(&(Ix, Ix), usize) -> f32>);

impl<Ix: Hash + Eq + Clone> LinkFn<Ix> {
    pub(crate) fn apply(&mut self, (i, link): (usize, &(Ix, Ix))) -> f32 {
        self.0(link, i)
    }
}

impl<Ix: Hash + Eq + Clone> From<f32> for LinkFn<Ix> {
    #[inline(always)]
    fn from(value: f32) -> Self {
        Self(Box::new(move |_, _| value))
    }
}

#[repr(transparent)]
pub struct NodeFn<Ix: Hash + Eq + Clone>(pub Box<dyn FnMut(&Ix, usize) -> f32>);

impl<Ix: Hash + Eq + Clone> NodeFn<Ix> {
    pub(crate) fn apply(&mut self, (i, node): (usize, &Ix)) -> f32 {
        self.0(node, i)
    }
}

impl<Ix: Hash + Eq + Clone> From<f32> for NodeFn<Ix> {
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
