use re_log_types::component_types::{Tensor, TensorDimension, TensorId};

// ----------------------------------------------------------------------------

/// A thin wrapper around a [`Tensor`] that is guaranteed to not be compressed (never a jpeg).
///
/// All clones are shallow, like for [`Tensor`].
#[derive(Clone)]
pub struct DecodedTensor(Tensor);

impl AsRef<Tensor> for DecodedTensor {
    #[inline(always)]
    fn as_ref(&self) -> &Tensor {
        &self.0
    }
}

impl std::ops::Deref for DecodedTensor {
    type Target = Tensor;

    #[inline(always)]
    fn deref(&self) -> &Tensor {
        &self.0
    }
}

impl std::borrow::Borrow<Tensor> for DecodedTensor {
    #[inline(always)]
    fn borrow(&self) -> &Tensor {
        &self.0
    }
}

// ----------------------------------------------------------------------------

#[derive(thiserror::Error, Clone, Debug)]
pub enum TensorDecodeError {
    // TODO(jleibs): It would be nice to just transparently wrap
    // `image::ImageError` and `tensor::TensorImageError` but neither implements
    // `Clone`, which we need if we want to cache the Result.
    #[error("Failed to decode bytes as tensor: {0}")]
    CouldNotDecode(String),

    #[error("Failed to interpret image as tensor: {0}")]
    InvalidImage(String),

    #[error("The encoded tensor did not match its metadata {expected:?} != {found:?}")]
    InvalidMetaData {
        expected: Vec<TensorDimension>,
        found: Vec<TensorDimension>,
    },
}

struct DecodedTensorResult {
    /// Cached `Result` from decoding the `Tensor`
    tensor: Result<DecodedTensor, TensorDecodeError>,

    /// Total memory used by this `Tensor`.
    memory_used: u64,

    /// Which [`DecodeCache::generation`] was this `Tensor` last used?
    last_use_generation: u64,
}

/// A cache of decoded [`Tensor`] entities, indexed by `TensorId`.
#[derive(Default)]
pub struct DecodeCache {
    images: nohash_hasher::IntMap<TensorId, DecodedTensorResult>,
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
    ) -> Result<DecodedTensor, TensorDecodeError> {
        crate::profile_function!();
        match &maybe_encoded_tensor.data {
            re_log_types::component_types::TensorData::JPEG(buf) => {
                let lookup = self
                    .images
                    .entry(maybe_encoded_tensor.id())
                    .or_insert_with(|| {
                        use image::io::Reader as ImageReader;
                        let mut reader = ImageReader::new(std::io::Cursor::new(buf.0.as_slice()));
                        reader.set_format(image::ImageFormat::Jpeg);
                        let img = {
                            crate::profile_scope!("decode_jpeg");
                            reader.decode()
                        };
                        let tensor = match img {
                            Ok(img) => match Tensor::from_image(img) {
                                Ok(tensor) => {
                                    if tensor.shape() == maybe_encoded_tensor.shape() {
                                        Ok(DecodedTensor(tensor))
                                    } else {
                                        Err(TensorDecodeError::InvalidMetaData {
                                            expected: maybe_encoded_tensor.shape().into(),
                                            found: tensor.shape().into(),
                                        })
                                    }
                                }
                                Err(err) => Err(TensorDecodeError::InvalidImage(err.to_string())),
                            },
                            Err(err) => Err(TensorDecodeError::CouldNotDecode(err.to_string())),
                        };

                        let memory_used = match &tensor {
                            Ok(tensor) => tensor.size_in_bytes() as u64,
                            Err(_) => 0,
                        };
                        self.memory_used += memory_used;
                        let last_use_generation = 0;
                        DecodedTensorResult {
                            tensor,
                            memory_used,
                            last_use_generation,
                        }
                    });
                lookup.last_use_generation = self.generation;

                lookup.tensor.clone()
            }
            _ => Ok(DecodedTensor(maybe_encoded_tensor)),
        }
    }

    /// Call once per frame to (potentially) flush the cache.
    pub fn begin_frame(&mut self, max_memory_use: u64) {
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
