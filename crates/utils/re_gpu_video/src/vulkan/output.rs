//! Decoded frames as wgpu textures.
//!
//! The output copy fills a new NV12 image per frame, which is handed to wgpu via
//! `texture_from_raw`/`create_texture_from_hal` and exposed through its two plane
//! views. The images come from a recycling pool: when wgpu is done with a texture
//! (the frame and its views dropped, all GPU work on it finished), the drop callback
//! returns the underlying image to the pool.

use std::sync::Arc;

use ash::vk;
use parking_lot::Mutex;

use crate::{ColorProperties, DecodedFrame};

use super::caps::QueuePlan;
use super::device::Device;
use super::record::CopySource;

/// wgpu-hal opens its own queue on family 0, so that is the family wgpu's
/// submissions run on and the output images must be shared with.
const WGPU_QUEUE_FAMILY: u32 = 0;

/// Recycled images beyond this stay out of the pool and free their memory instead.
const MAX_FREE_IMAGES: usize = 8;

/// One NV12 image from the pool, the output copy's destination.
pub(super) struct OutputImage {
    image: super::alloc::Image,

    /// The image's extent: the display size rounded up to even.
    pub extent: vk::Extent2D,
}

impl OutputImage {
    pub fn raw(&self) -> vk::Image {
        self.image.raw
    }
}

/// Creates and recycles the NV12 images decoded frames are handed to wgpu in.
pub(super) struct OutputPool {
    device: Arc<Device>,

    /// Images returned by dropped frames, available for reuse.
    /// Shared with the drop callbacks of the outstanding textures.
    free: Arc<Mutex<Vec<OutputImage>>>,

    /// The queue family the output copy runs on.
    copy_family: u32,
}

impl OutputPool {
    pub fn new(device: Arc<Device>, queue_plan: &QueuePlan) -> Self {
        let copy_family = queue_plan
            .copy
            .map_or(queue_plan.decode.family_index, |copy| copy.family_index);
        Self {
            device,
            free: Arc::new(Mutex::new(Vec::new())),
            copy_family,
        }
    }

    /// A new or recycled image fitting the frame. Its content is undefined.
    pub fn acquire(&self, source: &CopySource) -> Result<OutputImage, vk::Result> {
        // NV12 images need even sizes. `DecodedFrame` reports the true display
        // size, the padding row/column is never shown.
        let extent = vk::Extent2D {
            width: source.display[0].next_multiple_of(2),
            height: source.display[1].next_multiple_of(2),
        };

        {
            let mut free = self.free.lock();
            // Images of another size are stale (the stream changed resolution), drop them.
            free.retain(|image| image.extent == extent);
            if let Some(image) = free.pop() {
                return Ok(image);
            }
        }

        self.create_image(extent)
    }

    /// Puts an image that was never handed to wgpu back into the free list.
    ///
    /// All GPU work on it must have completed.
    pub fn recycle(&self, image: OutputImage) {
        let mut free = self.free.lock();
        if free.len() < MAX_FREE_IMAGES {
            free.push(image);
        }
    }

    fn create_image(&self, extent: vk::Extent2D) -> Result<OutputImage, vk::Result> {
        re_tracing::profile_function!();

        // Written by the copy queue, sampled by wgpu's queue: CONCURRENT sharing
        // between the two families avoids ownership transfers, which wgpu has
        // no way to take part in.
        let concurrent_families = [self.copy_family, WGPU_QUEUE_FAMILY];
        let (sharing_mode, families) = if self.copy_family == WGPU_QUEUE_FAMILY {
            (vk::SharingMode::EXCLUSIVE, &[][..])
        } else {
            (vk::SharingMode::CONCURRENT, &concurrent_families[..])
        };

        // The same flags wgpu itself creates multi-planar images with,
        // needed for the per-plane views on the wgpu side.
        let create_info = vk::ImageCreateInfo::default()
            .flags(vk::ImageCreateFlags::MUTABLE_FORMAT | vk::ImageCreateFlags::EXTENDED_USAGE)
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::G8_B8R8_2PLANE_420_UNORM)
            .extent(vk::Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(sharing_mode)
            .queue_family_indices(families)
            .initial_layout(vk::ImageLayout::UNDEFINED);

        let image = super::alloc::Image::new(self.device.clone(), &create_info)?;
        Ok(OutputImage { image, extent })
    }

    /// Hands a filled image to wgpu as an NV12 texture and builds the frame around
    /// its plane views.
    ///
    /// The image must have been left in `TRANSFER_DST_OPTIMAL` by the output copy,
    /// with the copy's completion observed on the host: the texture enters wgpu
    /// in the `COPY_DST` state and wgpu's own barriers take over from there.
    #[expect(unsafe_code)]
    pub fn wrap(
        &self,
        image: OutputImage,
        display: [u32; 2],
        pts: i64,
        is_idr: bool,
        color: ColorProperties,
    ) -> DecodedFrame {
        re_tracing::profile_function!();

        let size = wgpu::Extent3d {
            width: image.extent.width,
            height: image.extent.height,
            depth_or_array_layers: 1,
        };
        let usage = wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING;

        // When wgpu destroys the texture (frame and views dropped, GPU work on it
        // finished), the image goes back into the pool.
        let raw_image = image.raw();
        let free = Arc::clone(&self.free);
        let drop_callback = Box::new(move || {
            let mut free = free.lock();
            if free.len() < MAX_FREE_IMAGES {
                free.push(image);
            }
            // Beyond the cap the image drops here, freeing its memory.
        });

        let wgpu_device = &self.device.wgpu_device;
        // SAFETY: The image was created on this device with a descriptor matching the one
        // given here, its content is fully written (completion observed on the host), and
        // ownership (including deferred destruction) is handed over via the drop callback.
        // `TextureUses::COPY_DST` matches the `TRANSFER_DST_OPTIMAL` layout the copy left
        // the image in.
        let texture = unsafe {
            let hal_device = wgpu_device
                .as_hal::<wgpu::hal::api::Vulkan>()
                .expect("the video context only exists for Vulkan devices");
            let hal_texture = hal_device.texture_from_raw(
                raw_image,
                &wgpu::hal::TextureDescriptor {
                    label: Some("video decode output"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::NV12,
                    usage: wgpu::TextureUses::COPY_DST | wgpu::TextureUses::RESOURCE,
                    memory_flags: wgpu::hal::MemoryFlags::empty(),
                    view_formats: Vec::new(),
                },
                Some(drop_callback),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            drop(hal_device);

            wgpu_device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                hal_texture,
                &wgpu::TextureDescriptor {
                    label: Some("video decode output"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::NV12,
                    usage,
                    view_formats: &[],
                },
                wgpu::TextureUses::COPY_DST,
            )
        };

        let plane_view = |label, aspect, format| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(label),
                format: Some(format),
                aspect,
                ..Default::default()
            })
        };
        let y = plane_view(
            "video decode output (luma)",
            wgpu::TextureAspect::Plane0,
            wgpu::TextureFormat::R8Unorm,
        );
        let uv = plane_view(
            "video decode output (chroma)",
            wgpu::TextureAspect::Plane1,
            wgpu::TextureFormat::Rg8Unorm,
        );

        DecodedFrame::new(texture, y, uv, display[0], display[1], pts, is_idr, color)
    }
}
