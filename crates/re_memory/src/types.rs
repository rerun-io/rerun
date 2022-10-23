use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Default)]
pub struct Summary {
    /// Bytes allocated in non-shared memory (e.g. `capacity` of a `Vec<u8>`).
    pub allocated_capacity: usize,

    /// Bytes used in non-shared memory (e.g. `len` of a `Vec<u8>`).
    pub used: usize,

    /// Bytes allocated in shared data (Arc:s/Rc:s)
    pub shared: usize,

    /// Number of allocations made by this, excluding shared memory.
    pub num_allocs: usize,
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub type_name: &'static str,
    pub fields: Vec<(&'static str, Node)>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Summary(Summary),
    Struct(Struct),
}

#[derive(Clone, Copy, Debug)]
pub struct RefCountedInfo {
    pub strong_count: usize,
    pub summary: Summary,
}

/// Types which generate their own [`Node`] describing them
pub trait GenNode {
    fn node(&self, global: &mut Global) -> Node;
}

pub trait SumUp {
    fn sum_up(&self, global: &mut Global, summary: &mut Summary);

    fn summary(&self, global: &mut Global) -> Summary {
        let mut summary = Summary::default();
        self.sum_up(global, &mut summary);
        summary
    }
}

/// Tracks shared heap allocations
#[derive(Debug, Default)]
pub struct Global {
    pub ref_counted: HashMap<&'static str, HashMap<*const (), RefCountedInfo>>,
}

impl Global {
    /// Something shared by e.g. an [`Arc`] or [`std::cell::Rc`].
    ///
    /// Only summed up first time this is encountered.
    pub fn sum_up_shared(
        &mut self,
        type_name: &'static str,
        ptr: *const (),
        strong_count: usize,
        sum_up: &dyn SumUp,
    ) {
        {
            if let Some(ref_counted) = self.ref_counted.entry(type_name).or_default().get_mut(&ptr)
            {
                ref_counted.strong_count = ref_counted.strong_count.max(strong_count);
                return;
            }
        }

        let summary = sum_up.summary(self);
        self.ref_counted.entry(type_name).or_default().insert(
            ptr,
            RefCountedInfo {
                strong_count,
                summary,
            },
        );
    }

    pub fn sum_up_arc<T>(&mut self, arc: &Arc<T>)
    where
        T: SumUp,
    {
        self.sum_up_shared(
            std::any::type_name::<T>(),
            Arc::as_ptr(arc).cast(),
            Arc::strong_count(arc),
            &**arc,
        );
    }
}
