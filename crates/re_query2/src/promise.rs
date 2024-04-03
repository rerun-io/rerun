use std::sync::Arc;

use re_log_types::DataCell;

// ---

/// Uniquely identifies a [`Promise`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PromiseId(pub(crate) re_tuid::Tuid);

impl std::fmt::Display for PromiseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl re_types_core::SizeBytes for PromiseId {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

impl PromiseId {
    /// Create a new unique [`PromiseId`] based on the current time.
    #[allow(clippy::new_without_default)]
    #[inline]
    pub fn new() -> Self {
        Self(re_tuid::Tuid::new())
    }
}

// ---

/// A [`Promise`] turns a source [`DataCell`] into a new [`DataCell`] with the helper of a
/// [`PromiseResolver`].
///
/// Each promise is uniquely identified via a [`PromiseId`].
///
/// [`Promise`]s can be cloned cheaply.
#[derive(Debug, Clone)]
pub struct Promise {
    id: PromiseId,
    source: DataCell,
}

static_assertions::assert_eq_size!(Promise, Option<Promise>);

impl re_types_core::SizeBytes for Promise {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self { id, source } = self;
        id.heap_size_bytes() + source.heap_size_bytes()
    }
}

impl Promise {
    #[inline]
    pub fn new(source: DataCell) -> Self {
        Self {
            id: PromiseId::new(),
            source,
        }
    }
}

// ---

/// Resolves and keeps track of [`Promise`]s.
#[derive(Default)]
pub struct PromiseResolver {}

impl PromiseResolver {
    /// Resolves the given [`Promise`].
    ///
    /// If the method returns [`PromiseResult::Pending`], you should call it again with the same
    /// [`Promise`] until it either resolves it or fails irrecoverably.
    ///
    /// Once a [`Promise`] has left the `Pending` state, `resolve`ing it again is cached and
    /// idempotent (the [`PromiseResolver`] keeps track of the state of all [`Promise`]s, both
    /// pending and already resolved).
    #[inline]
    pub fn resolve(&self, promise: &Promise) -> PromiseResult<DataCell> {
        // NOTE: we're pretending there's gonna be some kind of interior mutability when
        // everything's said and done.
        _ = self;
        _ = promise.id;
        PromiseResult::Ready(promise.source.clone())
    }
}

/// The result of resolving a [`Promise`] through a [`PromiseResolver`].
#[derive(Clone)]
pub enum PromiseResult<T> {
    /// The resolution process is still in progress.
    ///
    /// Try calling [`PromiseResolver::resolve`] again.
    Pending,

    /// The [`Promise`] failed to resolve due to an irrecoverable error.
    Error(Arc<dyn std::error::Error + Send + Sync>),

    /// The [`Promise`] has been fully resolved.
    Ready(T),
}

impl<T> PromiseResult<T> {
    /// Applies the given transformation to the [`PromiseResult`] iff it's `Ready`.
    #[inline]
    pub fn map<B, F>(self, mut f: F) -> PromiseResult<B>
    where
        F: FnMut(T) -> B,
    {
        match self {
            PromiseResult::Ready(v) => PromiseResult::Ready(f(v)),
            PromiseResult::Pending => PromiseResult::Pending,
            PromiseResult::Error(err) => PromiseResult::Error(err),
        }
    }

    /// Applies the given transformation to the [`PromiseResult`] iff it's `Ready`.
    ///
    /// Able to modify the result itself, not just the value contained within.
    #[inline]
    pub fn remap<B, F>(self, mut f: F) -> PromiseResult<B>
    where
        F: FnMut(T) -> PromiseResult<B>,
    {
        match self {
            PromiseResult::Ready(v) => f(v),
            PromiseResult::Pending => PromiseResult::Pending,
            PromiseResult::Error(err) => PromiseResult::Error(err),
        }
    }

    /// Returns the inner value if it's ready.
    #[inline]
    pub fn ok(self) -> Option<T> {
        match self {
            PromiseResult::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// Unwraps the resolved result if it's `Ready`, panics otherwise.
    #[inline]
    pub fn unwrap(self) -> T {
        match self {
            PromiseResult::Ready(v) => v,
            PromiseResult::Pending => panic!("tried to unwrap a pending `PromiseResult`"),
            PromiseResult::Error(err) => {
                panic!("tried to unwrap an errored `PromiseResult`: {err}")
            }
        }
    }
}

impl<T, E: 'static + std::error::Error + Send + Sync> PromiseResult<Result<T, E>> {
    /// Given a [`PromiseResult`] of a `Result`, flattens it down to a single layer [`PromiseResult`].
    #[inline]
    pub fn flatten(self) -> PromiseResult<T> {
        self.remap(|res| match res {
            Ok(v) => PromiseResult::Ready(v),
            Err(err) => PromiseResult::Error(Arc::new(err) as _),
        })
    }
}
