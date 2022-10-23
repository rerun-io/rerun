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

impl Summary {
    #[inline]
    pub fn from_fixed(bytes: usize) -> Self {
        Self {
            allocated_capacity: bytes,
            used: bytes,
            shared: 0,
            num_allocs: 0,
        }
    }

    #[inline]
    pub fn add_fixed(&mut self, bytes: usize) {
        self.allocated_capacity += bytes;
        self.used += bytes;
    }
}

impl std::ops::AddAssign for Summary {
    fn add_assign(&mut self, rhs: Self) {
        self.allocated_capacity += rhs.allocated_capacity;
        self.used += rhs.used;
        self.shared += rhs.shared;
        self.num_allocs += rhs.num_allocs;
    }
}

#[derive(Clone, Debug, Default)]
pub struct Map {
    pub fields: Vec<(String, Node)>,
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub type_name: &'static str,
    pub fields: Vec<(&'static str, Node)>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Unknown,
    Summary(Summary),
    Map(Map),
    Struct(Struct),
}

#[macro_export]
macro_rules! impl_into_enum {
    ($from_ty: ty, $enum_name: ident, $to_enum_variant: ident) => {
        impl From<$from_ty> for $enum_name {
            #[inline]
            fn from(value: $from_ty) -> Self {
                Self::$to_enum_variant(value)
            }
        }
    };
}

impl_into_enum!(Summary, Node, Summary);
impl_into_enum!(Map, Node, Map);
impl_into_enum!(Struct, Node, Struct);

impl Default for Node {
    #[inline]
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Copy, Debug, Default)]
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

    #[inline]
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
    ///
    /// Returns byte used by the pointed-to value.
    pub fn sum_up_shared(
        &mut self,
        type_name: &'static str,
        ptr: *const (),
        strong_count: usize,
        sum_up: &dyn SumUp,
    ) -> usize {
        {
            if let Some(info) = self.ref_counted.entry(type_name).or_default().get_mut(&ptr) {
                info.strong_count = info.strong_count.max(strong_count);
                return info.summary.allocated_capacity + info.summary.shared;
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
        summary.allocated_capacity + summary.shared
    }

    pub fn add_shared_summary(
        &mut self,
        type_name: &'static str,
        ptr: *const (),
        strong_count: usize,
        summary: Summary,
    ) {
        let info = self
            .ref_counted
            .entry(type_name)
            .or_default()
            .entry(ptr)
            .or_default();
        info.strong_count = info.strong_count.max(strong_count);
        info.summary = summary;
    }

    /// Returns byte used by the pointed-to value.
    #[must_use]
    #[inline]
    pub fn sum_up_arc<T>(&mut self, arc: &Arc<T>) -> usize
    where
        T: SumUp,
    {
        self.sum_up_shared(
            std::any::type_name::<T>(),
            Arc::as_ptr(arc).cast(),
            Arc::strong_count(arc),
            &**arc,
        )
    }

    #[inline]
    #[must_use]
    pub fn sum_up_hash_map<K, V: SumUp, R>(
        &mut self,
        map: &std::collections::HashMap<K, V, R>,
    ) -> Summary {
        let bytes_per_key = std::mem::size_of::<K>();

        let mut summary = Summary {
            allocated_capacity: map.capacity() * bytes_per_key, // TODO: better estimate
            used: map.len() * bytes_per_key,
            shared: 0,
            num_allocs: (map.capacity() != 0) as _,
        };

        for value in map.values() {
            value.sum_up(self, &mut summary);
        }

        summary
    }
}
