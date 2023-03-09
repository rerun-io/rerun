use re_log_types::component_types::{Tensor, TensorId, TensorImageError, TensorTrait};

struct CachedTensor {
    /// `None` if the tensor was not successfully decoded
    tensor: Result<Tensor, TensorImageError>,

    /// Total memory used by this image.
    memory_used: u64,

    /// When [`ImageCache::generation`] was we last used?
    last_use_generation: u64,
}

#[derive(Default)]
pub struct DecodeCache {
    images: nohash_hasher::IntMap<TensorId, CachedTensor>,
    memory_used: u64,
    generation: u64,
}

impl DecodeCache {
    pub fn try_decode_tensor_if_necessary<'a>(
        &'a mut self,
        tensor: &'a Tensor,
    ) -> Result<&'a Tensor, &TensorImageError> {
        match &tensor.data {
            re_log_types::component_types::TensorData::JPEG(buf) => {
                let lookup = self.images.entry(tensor.id()).or_insert_with(|| {
                    use image::io::Reader as ImageReader;
                    let mut reader = ImageReader::new(std::io::Cursor::new(buf));
                    reader.set_format(image::ImageFormat::Jpeg);
                    // TODO(emilk): handle grayscale JPEG:s (depth == 1)
                    let img = {
                        crate::profile_scope!("decode_jpeg");
                        reader.decode()
                    };
                    let tensor = match img {
                        Ok(img) => Tensor::from_image(img),
                        Err(err) => Err(err.into()),
                    };

                    let memory_used = match &tensor {
                        Ok(tensor) => tensor.size_in_bytes() as u64,
                        Err(_) => 0,
                    };
                    let last_use_generation = 0;
                    CachedTensor {
                        tensor,
                        memory_used,
                        last_use_generation,
                    }
                });
                lookup.tensor.as_ref()
            }
            _ => Ok(tensor),
        }
    }

    pub fn purge_memory(&mut self) {
        crate::profile_function!();

        // Very aggressively flush everything not used in this frame

        let before = self.memory_used;

        self.images.retain(|_, ci| {
            let retain = ci.last_use_generation == self.generation;
            if !retain {
                self.memory_used -= ci.memory_used;
            }
            retain
        });

        re_log::debug!(
            "Flushed jpeg cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }
}
