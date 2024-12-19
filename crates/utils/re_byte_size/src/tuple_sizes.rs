use crate::SizeBytes;

impl<T, U> SizeBytes for (T, U)
where
    T: SizeBytes,
    U: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b) = self;
        a.heap_size_bytes() + b.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod()
    }
}

impl<T, U, V> SizeBytes for (T, U, V)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod() && V::is_pod()
    }
}

impl<T, U, V, W> SizeBytes for (T, U, V, W)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
    W: SizeBytes,
{
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c, d) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes() + d.heap_size_bytes()
    }

    #[inline]
    fn is_pod() -> bool {
        T::is_pod() && U::is_pod() && V::is_pod() && W::is_pod()
    }
}
