use crate::SizeBytes;

// TODO(rust-lang/rust#31844): This isn't happening without specialization.
// impl<T> SizeBytes for T where T: bytemuck::Pod { … }

// TODO(rust-lang/rust#31844): `impl<T: bytemuck::Pod> SizeBytesExt for T {}` would be nice but
// violates orphan rules.
macro_rules! impl_size_bytes_pod {
    ($ty:ty) => {
        impl SizeBytes for $ty {
            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }

            #[inline]
            fn is_pod() -> bool {
                true
            }
        }
    };
    ($ty:ty, $($rest:ty),+) => {
        impl_size_bytes_pod!($ty); impl_size_bytes_pod!($($rest),+);
    };
}

impl_size_bytes_pod!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, bool, f32, f64);
impl_size_bytes_pod!(half::f16);
