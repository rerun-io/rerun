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
struct CachedTensor {
    /// `None` if the tensor was not successfully decoded
    tensor: Result<Tensor, TensorDecodeError>,

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

#[allow(clippy::map_err_ignore)]
impl DecodeCache {
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
                        let last_use_generation = 0;
                        CachedTensor {
                            tensor,
                            memory_used,
                            last_use_generation,
                        }
                    });
                lookup.tensor.clone()
            }
            _ => Ok(maybe_encoded_tensor),
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
            "Flushed tensor decode cache. Before: {:.2} GB. After: {:.2} GB",
            before as f64 / 1e9,
            self.memory_used as f64 / 1e9,
        );
    }
}
