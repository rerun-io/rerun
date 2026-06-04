use crate::SizeBytes;

impl SizeBytes for () {
    const IS_POD: bool = true;

    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl<T, U> SizeBytes for (T, U)
where
    T: SizeBytes,
    U: SizeBytes,
{
    const IS_POD: bool = T::IS_POD && U::IS_POD;

    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b) = self;
        a.heap_size_bytes() + b.heap_size_bytes()
    }
}

impl<T, U, V> SizeBytes for (T, U, V)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
{
    const IS_POD: bool = T::IS_POD && U::IS_POD && V::IS_POD;

    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes()
    }
}

impl<T, U, V, W> SizeBytes for (T, U, V, W)
where
    T: SizeBytes,
    U: SizeBytes,
    V: SizeBytes,
    W: SizeBytes,
{
    const IS_POD: bool = T::IS_POD && U::IS_POD && V::IS_POD && W::IS_POD;

    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let (a, b, c, d) = self;
        a.heap_size_bytes() + b.heap_size_bytes() + c.heap_size_bytes() + d.heap_size_bytes()
    }
}
