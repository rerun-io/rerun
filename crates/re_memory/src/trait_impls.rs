use crate::*;

impl SumUp for u8 {
    #[inline]
    fn sum_up(&self, _global: &mut Global, summary: &mut Summary) {
        summary.allocated_capacity += 1;
        summary.used += 1;
    }
}

impl SumUp for Vec<u8> {
    #[inline]
    fn sum_up(&self, _global: &mut Global, summary: &mut Summary) {
        summary.allocated_capacity += std::mem::size_of_val(self) + self.capacity();
        summary.used += std::mem::size_of_val(self) + self.len();
        summary.num_allocs += (self.capacity() != 0) as usize;
    }
}

impl SumUp for String {
    #[inline]
    fn sum_up(&self, _global: &mut Global, summary: &mut Summary) {
        summary.allocated_capacity += std::mem::size_of_val(self) + self.capacity();
        summary.used += std::mem::size_of_val(self) + self.len();
        summary.num_allocs += (self.capacity() != 0) as usize;
    }
}

impl<T: SumUp> SumUp for std::sync::Arc<T> {
    #[inline]
    fn sum_up(&self, global: &mut Global, summary: &mut Summary) {
        summary.allocated_capacity += std::mem::size_of_val(self);
        summary.used += std::mem::size_of_val(self);
        summary.shared += global.sum_up_arc(self);
    }
}

// ----------------------------------------------------------------------------

impl GenNode for Vec<u8> {
    #[inline]
    fn node(&self, global: &mut Global) -> Node {
        Node::Summary(self.summary(global))
    }
}

impl GenNode for String {
    #[inline]
    fn node(&self, global: &mut Global) -> Node {
        Node::Summary(self.summary(global))
    }
}

impl<T: SumUp> GenNode for std::sync::Arc<T> {
    #[inline]
    fn node(&self, global: &mut Global) -> Node {
        Node::Summary(self.summary(global))
    }
}
