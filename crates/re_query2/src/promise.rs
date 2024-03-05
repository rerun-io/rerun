use std::sync::Arc;

use ahash::HashMap;
use re_log_types::DataCell;

// ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PromiseId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for PromiseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl PromiseId {
    pub const UNINIT: Self = Self(re_tuid::Tuid::ZERO);

    /// Create a new unique [`PromiseId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }

    /// Returns the next logical [`PromiseId`].
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`PromiseId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn next(&self) -> Self {
        Self(self.0.next())
    }

    /// Returns the `n`-next logical [`PromiseId`].
    ///
    /// This is equivalent to calling [`PromiseId::next`] `n` times.
    /// Wraps the monotonically increasing back to zero on overflow.
    ///
    /// Beware: wrong usage can easily lead to conflicts.
    /// Prefer [`PromiseId::new`] when unsure.
    #[must_use]
    #[inline]
    pub fn incremented_by(&self, n: u64) -> Self {
        Self(self.0.incremented_by(n))
    }

    /// When the `PromiseId` was created, in nanoseconds since unix epoch.
    #[inline]
    pub fn nanoseconds_since_epoch(&self) -> u64 {
        self.0.nanoseconds_since_epoch()
    }
}

// ---

// TODO: should we actually store eraseddeques in there? what are the problems with that when it
// comes to ranges?

#[derive(Default)]
pub struct PromiseResolver {
    promises: HashMap<PromiseId, PromiseResult<DataCell>>,
}

impl PromiseResolver {
    pub fn resolve(&mut self, promise: &Promise) -> PromiseResult<DataCell> {
        // TODO: things

        // let Some(source) = promise.source.as_ref() else {
        //     return PromiseResult::Uninit;
        // };

        let result = self
            .promises
            .entry(promise.id)
            .or_insert_with(|| PromiseResult::Ready(promise.source.clone()));

        result.clone()
    }
}

// pub struct Promise<T>(T);
//
// // TODO: let's implement it in a way that it is always ready
// impl<T> Promise<T> {
//     pub fn resolve<'cache>(&self, cache: &'cache PromiseCache) -> PromiseResult<'cache, T> {}
// }

#[derive(Clone)]
pub enum PromiseResult<T> {
    // #[default]
    // Uninit,

    // TODO
    // PermanentlyErrored,
    Pending,

    Error(Arc<dyn std::error::Error + Send + Sync>),

    Ready(T),
}

impl<T> PromiseResult<T> {
    #[inline]
    pub fn map<B, F>(self, mut f: F) -> PromiseResult<B>
    where
        F: FnMut(T) -> PromiseResult<B>,
    {
        match self {
            PromiseResult::Ready(v) => f(v),
            // PromiseResult::Uninit => PromiseResult::Uninit,
            PromiseResult::Pending => PromiseResult::Pending,
            PromiseResult::Error(err) => PromiseResult::Error(err),
        }
    }

    #[inline]
    pub fn map_ok<B, F>(self, mut f: F) -> PromiseResult<B>
    where
        F: FnMut(T) -> B,
    {
        match self {
            PromiseResult::Ready(v) => PromiseResult::Ready(f(v)),
            // PromiseResult::Uninit => PromiseResult::Uninit,
            PromiseResult::Pending => PromiseResult::Pending,
            PromiseResult::Error(err) => PromiseResult::Error(err),
        }
    }

    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            PromiseResult::Ready(v) => v,
            PromiseResult::Pending => panic!("pending"),
            PromiseResult::Error(err) => panic!("{err}"),
        }
    }
}

impl<T, E: 'static + std::error::Error + Send + Sync> PromiseResult<Result<T, E>> {
    #[inline]
    pub fn flatten(self) -> PromiseResult<T> {
        match self {
            PromiseResult::Ready(res) => match res {
                Ok(v) => PromiseResult::Ready(v),
                Err(err) => PromiseResult::Error(Arc::new(err) as _),
            },
            // PromiseResult::Uninit => PromiseResult::Uninit,
            PromiseResult::Pending => PromiseResult::Pending,
            PromiseResult::Error(err) => PromiseResult::Error(err),
        }
    }
}

// TODO: pretty sure promise gotta be a trait then

// TODO: for now we don't really care how this trait looks -- it'll evolve as needed

#[derive(Debug, Clone)]
pub struct Promise {
    id: PromiseId,
    // source: Option<DataCell>,
    source: DataCell,
}

impl Promise {
    #[inline]
    pub fn new(source: DataCell) -> Self {
        Self {
            id: PromiseId::new(),
            source,
        }
    }

    // #[inline]
    // pub const fn uninit() -> Self {
    //     Self {
    //         id: PromiseId::UNINIT,
    //         source: None,
    //     }
    // }
}

// impl Promise for Promise {
//     type Output = DataCell;
//
//     fn resolve(&mut self, cache: &PromiseCache) -> &PromiseResult<Self::Output> {
//         // TODO: somehow this needs to do work and then hand it back later..?
//
//         &self.resolved
//     }
// }
