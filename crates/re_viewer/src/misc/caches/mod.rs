mod mesh_cache;
mod tensor_image_cache;

use re_log_types::component_types;
pub use tensor_image_cache::{AsDynamicImage, TensorImageView};

/// Does memoization of different things for the immediate mode UI.
#[derive(Default)]
pub struct Caches {
    /// For displaying images efficiently in immediate mode.
    pub image: tensor_image_cache::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    pub mesh: mesh_cache::MeshCache,

    pub tensor_stats: nohash_hasher::IntMap<component_types::TensorId, TensorStats>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn new_frame(&mut self) {
        let max_image_cache_use = 1_000_000_000_000;
        self.image.new_frame(max_image_cache_use);
    }

    pub fn purge_memory(&mut self) {
        let Self {
            image,
            tensor_stats,
            mesh: _, // TODO(emilk)
        } = self;
        image.purge_memory();
        tensor_stats.clear();
    }

    pub fn tensor_stats(&mut self, tensor: &re_log_types::ClassicTensor) -> &TensorStats {
        self.tensor_stats
            .entry(tensor.id())
            .or_insert_with(|| TensorStats::new(tensor))
    }
}

pub struct TensorStats {
    pub range: Option<(f64, f64)>,
}

impl TensorStats {
    fn new(tensor: &re_log_types::ClassicTensor) -> Self {
        use re_log_types::TensorDataType;
        use re_tensor_ops::as_ndarray;

        use half::f16;

        macro_rules! declare_tensor_range_int {
            ($name: ident, $typ: ty) => {
                fn $name(tensor: ndarray::ArrayViewD<'_, $typ>) -> (f64, f64) {
                    crate::profile_function!();
                    let (min, max) = tensor
                        .fold((<$typ>::MAX, <$typ>::MIN), |(min, max), &value| {
                            (min.min(value), max.max(value))
                        });
                    (min as f64, max as f64)
                }
            };
        }

        macro_rules! declare_tensor_range_float {
            ($name: ident, $typ: ty) => {
                fn $name(tensor: ndarray::ArrayViewD<'_, $typ>) -> (f64, f64) {
                    crate::profile_function!();
                    let (min, max) = tensor.fold(
                        (<$typ>::INFINITY, <$typ>::NEG_INFINITY),
                        |(min, max), &value| (min.min(value), max.max(value)),
                    );
                    #[allow(trivial_numeric_casts)]
                    (min as f64, max as f64)
                }
            };
        }

        declare_tensor_range_int!(tensor_range_u8, u8);
        declare_tensor_range_int!(tensor_range_u16, u16);
        declare_tensor_range_int!(tensor_range_u32, u32);
        declare_tensor_range_int!(tensor_range_u64, u64);

        declare_tensor_range_int!(tensor_range_i8, i8);
        declare_tensor_range_int!(tensor_range_i16, i16);
        declare_tensor_range_int!(tensor_range_i32, i32);
        declare_tensor_range_int!(tensor_range_i64, i64);

        // declare_tensor_range_float!(tensor_range_f16, half::f16);
        declare_tensor_range_float!(tensor_range_f32, f32);
        declare_tensor_range_float!(tensor_range_f64, f64);

        #[allow(clippy::needless_pass_by_value)]
        fn tensor_range_f16(tensor: ndarray::ArrayViewD<'_, f16>) -> (f64, f64) {
            crate::profile_function!();
            let (min, max) = tensor
                .fold((f16::INFINITY, f16::NEG_INFINITY), |(min, max), &value| {
                    (min.min(value), max.max(value))
                });
            (min.to_f64(), max.to_f64())
        }

        let range = match tensor.dtype() {
            TensorDataType::U8 => as_ndarray::<u8>(tensor).ok().map(tensor_range_u8),
            TensorDataType::U16 => as_ndarray::<u16>(tensor).ok().map(tensor_range_u16),
            TensorDataType::U32 => as_ndarray::<u32>(tensor).ok().map(tensor_range_u32),
            TensorDataType::U64 => as_ndarray::<u64>(tensor).ok().map(tensor_range_u64),

            TensorDataType::I8 => as_ndarray::<i8>(tensor).ok().map(tensor_range_i8),
            TensorDataType::I16 => as_ndarray::<i16>(tensor).ok().map(tensor_range_i16),
            TensorDataType::I32 => as_ndarray::<i32>(tensor).ok().map(tensor_range_i32),
            TensorDataType::I64 => as_ndarray::<i64>(tensor).ok().map(tensor_range_i64),

            TensorDataType::F16 => as_ndarray::<f16>(tensor).ok().map(tensor_range_f16),
            TensorDataType::F32 => as_ndarray::<f32>(tensor).ok().map(tensor_range_f32),
            TensorDataType::F64 => as_ndarray::<f64>(tensor).ok().map(tensor_range_f64),
        };

        Self { range }
    }
}
