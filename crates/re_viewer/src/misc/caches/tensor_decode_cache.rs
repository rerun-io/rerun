use re_log_types::component_types::{Tensor, TensorId, TensorTrait};

#[derive(thiserror::Error, Clone, Debug)]
pub enum TensorDecodeError {
    // TODO(jleibs): It would be nice to just transparently wrap
    // `image::ImageError` and `tensor::TensorImageError` but neither implements
    // `Clone`, which we need if we ant to cache the Result.
    #[error("Failed to decode bytes as tensor")]
    CouldNotDecode,
    #[error("Failed to interpret image as tensor")]
    InvalidImage,
    #[error("The encoded tensor did not match its metadata")]
    InvalidMetaData,
}

#[derive(Clone)]
struct DecodedTensor {
    /// `None` if the tensor was not successfully decoded
    tensor: Result<Tensor, TensorDecodeError>,

    /// Total memory used by this image.
    memory_used: u64,

    /// When [`ImageCache::generation`] was we last used?
    last_use_generation: u64,
}

/// A cache of decoded [`Tensor`] entities, indexed by `TensorId`.
#[derive(Default)]
pub struct DecodeCache {
    images: nohash_hasher::IntMap<TensorId, DecodedTensor>,
    memory_used: u64,
    generation: u64,
}

#[allow(clippy::map_err_ignore)]
impl DecodeCache {
    /// Decode a [`Tensor`] if necessary and cache the result.
    ///
    /// This is a no-op for Tensors that are not compressed.
    ///
    /// Currently supports JPEG encoded tensors.
    pub fn try_decode_tensor_if_necessary(
        &mut self,
        maybe_encoded_tensor: Tensor,
    ) -> Result<Tensor, TensorDecodeError> {
        match &maybe_encoded_tensor.data {
            re_log_types::component_types::TensorData::JPEG(buf) => {
                let lookup = self
                    .images
                    .entry(maybe_encoded_tensor.id())
                    .or_insert_with(|| {
                        use image::io::Reader as ImageReader;
                        let mut reader = ImageReader::new(std::io::Cursor::new(buf));
                        reader.set_format(image::ImageFormat::Jpeg);
                        let img = {
                            crate::profile_scope!("decode_jpeg");
                            reader.decode()
                        };
                        let tensor = match img {
                            Ok(img) => match Tensor::from_image(img) {
                                Ok(tensor) => {
                                    if tensor.shape() == maybe_encoded_tensor.shape() {
                                        Ok(tensor)
                                    } else {
                                        Err(TensorDecodeError::InvalidMetaData)
                                    }
                                }
                                Err(_) => Err(TensorDecodeError::InvalidImage),
                            },
                            Err(_) => Err(TensorDecodeError::CouldNotDecode),
                        };

                        let memory_used = match &tensor {
                            Ok(tensor) => tensor.size_in_bytes() as u64,
                            Err(_) => 0,
                        };
                        self.memory_used += memory_used;
                        let last_use_generation = 0;
                        DecodedTensor {
                            tensor,
                            memory_used,
                            last_use_generation,
                        }
                    });
                lookup.last_use_generation = self.generation;
                lookup.tensor.clone()
            }
            _ => Ok(maybe_encoded_tensor),
        }
    }

    /// Call once per frame to (potentially) flush the cache.
    pub fn new_frame(&mut self, max_memory_use: u64) {
        // TODO(jleibs): a more incremental purging mechanism, maybe switching to an LRU Cache
        // would likely improve the behavior.

        if self.memory_used > max_memory_use {
            self.purge_memory();
        }

        self.generation += 1;
    }

    /// Attempt to free up memory.
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
            "Flushed tensor decode cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }
}
