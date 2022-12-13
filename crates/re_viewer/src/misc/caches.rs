#[derive(Default)]
pub struct Caches {
    /// For displaying images efficiently in immediate mode.
    pub image: crate::misc::ImageCache,

    /// For displaying meshes efficiently in immediate mode.
    pub cpu_mesh: crate::ui::view_spatial::CpuMeshCache,

    pub tensor_stats: nohash_hasher::IntMap<re_log_types::TensorId, TensorStats>,
}

impl Caches {
    /// Call once per frame to potentially flush the cache(s).
    pub fn new_frame(&mut self) {
        let max_image_cache_use = 1_000_000_000;
        self.image.new_frame(max_image_cache_use);
    }

    pub fn purge_memory(&mut self) {
        let Self {
            image,
            tensor_stats,
            cpu_mesh: _, // TODO(emilk)
        } = self;
        image.purge_memory();
        tensor_stats.clear();
    }

    pub fn tensor_stats(&mut self, tensor: &re_log_types::Tensor) -> &TensorStats {
        self.tensor_stats
            .entry(tensor.tensor_id)
            .or_insert_with(|| TensorStats::new(tensor))
    }
}

pub struct TensorStats {
    pub range: Option<(f64, f64)>,
}

impl TensorStats {
    fn new(tensor: &re_log_types::Tensor) -> Self {
        use re_log_types::TensorDataType;

        fn tensor_range_f32(tensor: &ndarray::ArrayViewD<'_, f32>) -> (f32, f32) {
            crate::profile_function!();
            tensor.fold((f32::INFINITY, f32::NEG_INFINITY), |cur, &value| {
                (cur.0.min(value), cur.1.max(value))
            })
        }

        fn tensor_range_u16(tensor: &ndarray::ArrayViewD<'_, u16>) -> (u16, u16) {
            crate::profile_function!();
            tensor.fold((u16::MAX, u16::MIN), |cur, &value| {
                (cur.0.min(value), cur.1.max(value))
            })
        }

        let range = match tensor.dtype {
            TensorDataType::U8 => Some((0.0, 255.0)),

            TensorDataType::U16 => re_tensor_ops::as_ndarray::<u16>(tensor).ok().map(|tensor| {
                let (min, max) = tensor_range_u16(&tensor);
                (min as f64, max as f64)
            }),

            TensorDataType::F32 => re_tensor_ops::as_ndarray::<f32>(tensor).ok().map(|tensor| {
                let (min, max) = tensor_range_f32(&tensor);
                (min as f64, max as f64)
            }),
        };

        Self { range }
    }
}
