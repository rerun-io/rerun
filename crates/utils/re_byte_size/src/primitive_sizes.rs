use crate::SizeBytes;

// TODO(rust-lang/rust#31844): This isn't happening without specialization.
// impl<T> SizeBytes for T where T: bytemuck::Pod { … }

// TODO(rust-lang/rust#31844): `impl<T: bytemuck::Pod> SizeBytesExt for T {}` would be nice but
// violates orphan rules.
macro_rules! impl_size_bytes_pod {
    ($ty:ty) => {
        impl SizeBytes for $ty {
            const IS_POD: bool = true;

            #[inline]
            fn heap_size_bytes(&self) -> u64 {
                0
            }
        }
    };
    ($ty:ty, $($rest:ty),+) => {
        impl_size_bytes_pod!($ty); impl_size_bytes_pod!($($rest),+);
    };
}

impl_size_bytes_pod!(
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    bool,
    f32,
    f64,
    std::num::NonZeroU8,
    std::num::NonZeroU16,
    std::num::NonZeroU32,
    std::num::NonZeroU64,
    std::num::NonZeroU128,
    std::num::NonZeroUsize,
    std::num::NonZeroI8,
    std::num::NonZeroI16,
    std::num::NonZeroI32,
    std::num::NonZeroI64,
    std::num::NonZeroI128,
    std::num::NonZeroIsize,
    std::sync::atomic::AtomicU8,
    std::sync::atomic::AtomicU16,
    std::sync::atomic::AtomicU32,
    std::sync::atomic::AtomicU64,
    std::sync::atomic::AtomicUsize,
    std::sync::atomic::AtomicI8,
    std::sync::atomic::AtomicI16,
    std::sync::atomic::AtomicI32,
    std::sync::atomic::AtomicI64,
    std::sync::atomic::AtomicIsize,
    std::sync::atomic::AtomicBool,
    &'static str,
    &'static [u8],
    std::time::Duration
);
impl_size_bytes_pod!(half::f16);

#[cfg(feature = "ecolor")]
impl_size_bytes_pod!(ecolor::Color32);

#[cfg(feature = "egui")]
impl_size_bytes_pod!(egui::Id, egui::Pos2, egui::Rect, egui::Vec2);

#[cfg(feature = "glam")]
impl_size_bytes_pod!(
    glam::Mat3,
    glam::Quat,
    glam::Vec2,
    glam::Vec3,
    glam::DAffine3
);

#[cfg(feature = "macaw")]
impl_size_bytes_pod!(macaw::BoundingBox, macaw::IsoTransform);
