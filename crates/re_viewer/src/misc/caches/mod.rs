mod mesh_cache;
mod tensor_decode_cache;

use ahash::HashMap;
use re_log_types::component_types;

/// Does memoization of different things for the immediate mode UI.
#[derive(Default)]
pub struct Caches {
    /// Cached decoded tensors.
    pub decode: tensor_decode_cache::DecodeCache,

    /// Cached loaded meshes (from file or converted from user data).
    pub mesh: mesh_cache::MeshCache,

    tensor_stats: nohash_hasher::IntMap<component_types::TensorId, TensorStats>,

    caches: HashMap<std::any::TypeId, Box<dyn Cache + 'static>>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn begin_frame(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let max_decode_cache_use = 4_000_000_000;

        #[cfg(target_arch = "wasm32")]
        let max_decode_cache_use = 1_000_000_000;

        self.decode.begin_frame(max_decode_cache_use);

        for cache in self.caches.values_mut() {
            cache.begin_frame();
        }
    }

    pub fn purge_memory(&mut self) {
        let Self {
            decode,
            tensor_stats,
            mesh: _, // TODO(emilk)
            caches,
        } = self;
        decode.purge_memory();
        tensor_stats.clear();

        for cache in caches.values_mut() {
            cache.purge_memory();
        }
    }

    pub fn tensor_stats(&mut self, tensor: &re_log_types::component_types::Tensor) -> &TensorStats {
        self.tensor_stats
            .entry(tensor.tensor_id)
            .or_insert_with(|| TensorStats::new(tensor))
    }

    /// Retrieves a cache for reading and writing.
    ///
    /// Returns None if the cache is not present.
    pub fn get_mut<T: Cache>(&mut self) -> Option<&mut T> {
        self.caches
            .get_mut(&std::any::TypeId::of::<T>())
            .and_then(|cache| <dyn std::any::Any>::downcast_mut::<T>(cache))
    }

    /// Retrieves a cache for reading.
    ///
    /// Returns None if the cache is not present.
    pub fn get<T: Cache>(&self) -> Option<&T> {
        self.caches
            .get(&std::any::TypeId::of::<T>())
            .and_then(|cache| <dyn std::any::Any>::downcast_ref::<T>(cache))
    }

    /// Adds a cache to the list of caches.
    ///
    /// Fails if a cache of the same type already exists.
    pub fn add_cache<T: Cache>(&mut self, cache: T) -> Result<(), ()> {
        let type_id = std::any::TypeId::of::<T>();
        match self.caches.insert(type_id, Box::new(cache)) {
            Some(_) => Err(()),
            None => Ok(()),
        }
    }
}

/// A cache for memoizing things in order to speed up immediate mode UI & other immediate mode style things.
pub trait Cache: std::any::Any {
    /// Called once per frame to potentially flush the cache.
    fn begin_frame(&mut self);

    /// Attempt to free up memory.
    fn purge_memory(&mut self);
}

#[derive(Clone, Copy, Debug)]
pub struct TensorStats {
    /// This will currently only be `None` for jpeg-encoded tensors.
    pub range: Option<(f64, f64)>,
}

impl TensorStats {
    fn new(tensor: &re_log_types::component_types::Tensor) -> Self {
        use half::f16;
        use ndarray::ArrayViewD;
        use re_log_types::TensorDataType;

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
            TensorDataType::U8 => ArrayViewD::<u8>::try_from(tensor).map(tensor_range_u8),
            TensorDataType::U16 => ArrayViewD::<u16>::try_from(tensor).map(tensor_range_u16),
            TensorDataType::U32 => ArrayViewD::<u32>::try_from(tensor).map(tensor_range_u32),
            TensorDataType::U64 => ArrayViewD::<u64>::try_from(tensor).map(tensor_range_u64),

            TensorDataType::I8 => ArrayViewD::<i8>::try_from(tensor).map(tensor_range_i8),
            TensorDataType::I16 => ArrayViewD::<i16>::try_from(tensor).map(tensor_range_i16),
            TensorDataType::I32 => ArrayViewD::<i32>::try_from(tensor).map(tensor_range_i32),
            TensorDataType::I64 => ArrayViewD::<i64>::try_from(tensor).map(tensor_range_i64),
            TensorDataType::F16 => ArrayViewD::<f16>::try_from(tensor).map(tensor_range_f16),
            TensorDataType::F32 => ArrayViewD::<f32>::try_from(tensor).map(tensor_range_f32),
            TensorDataType::F64 => ArrayViewD::<f64>::try_from(tensor).map(tensor_range_f64),
        };

        Self { range: range.ok() }
    }
}
