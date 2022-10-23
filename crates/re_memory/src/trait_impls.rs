use crate::*;

#[macro_export]
macro_rules! impl_pod {
    ($pod: ty) => {
        impl $crate::SumUp for $pod {
            #[inline]
            fn sum_up(&self, _global: &mut $crate::Global, summary: &mut $crate::Summary) {
                summary.add_fixed(std::mem::size_of_val(self));
            }
        }
    };
}

impl_pod!(bool);
impl_pod!(char);
impl_pod!(u8);
impl_pod!(i8);
impl_pod!(u16);
impl_pod!(i16);
impl_pod!(u32);
impl_pod!(i32);
impl_pod!(u64);
impl_pod!(i64);
impl_pod!(u128);
impl_pod!(i128);
impl_pod!(f32);
impl_pod!(f64);

impl<const N: usize, T: SumUp> SumUp for [T; N] {
    #[inline]
    fn sum_up(&self, global: &mut Global, summary: &mut Summary) {
        for value in self {
            value.sum_up(global, summary);
        }
    }
}

impl<T: SumUp> SumUp for Vec<T> {
    #[inline]
    fn sum_up(&self, global: &mut Global, summary: &mut Summary) {
        summary.num_allocs += (self.capacity() != 0) as usize;
        for value in self {
            value.sum_up(global, summary);
        }
    }
}

// impl SumUp for Vec<u8> {
//     #[inline]
//     fn sum_up(&self, _global: &mut Global, summary: &mut Summary) {
//         summary.allocated_capacity += std::mem::size_of_val(self) + self.capacity();
//         summary.used += std::mem::size_of_val(self) + self.len();
//         summary.num_allocs += (self.capacity() != 0) as usize;
//     }
// }

impl SumUp for String {
    #[inline]
    fn sum_up(&self, _global: &mut Global, summary: &mut Summary) {
        summary.allocated_capacity += std::mem::size_of_val(self) + self.capacity();
        summary.used += std::mem::size_of_val(self) + self.len();
        summary.num_allocs += (self.capacity() != 0) as usize;
    }
}

// impl<T: SumUp> SumUp for std::sync::Arc<T> {
//     #[inline]
//     fn sum_up(&self, global: &mut Global, summary: &mut Summary) {
//         summary.allocated_capacity += std::mem::size_of_val(self);
//         summary.used += std::mem::size_of_val(self);
//         summary.shared += global.sum_up_arc(self);
//     }
// }

// ----------------------------------------------------------------------------

// impl GenNode for Vec<u8> {
//     #[inline]
//     fn node(&self, global: &mut Global) -> Node {
//         Node::Summary(self.summary(global))
//     }
// }

impl GenNode for String {
    #[inline]
    fn node(&self, global: &mut Global) -> Node {
        Node::Summary(self.summary(global))
    }
}

// impl<T: SumUp> GenNode for std::sync::Arc<T> {
//     #[inline]
//     fn node(&self, global: &mut Global) -> Node {
//         Node::Summary(self.summary(global))
//     }
// }
