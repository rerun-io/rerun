//! The decoded-picture-buffer images and the decode output image.
//!
//! DPB slots are layers of one NV12 array image (always legal, also on hardware
//! without `SEPARATE_REFERENCE_IMAGES`), with the parser's slot index doubling as
//! the layer index. On distinct-mode hardware decode output goes to a ring of
//! separate single-layer images, one per in-flight frame, so a decode never has
//! to wait for the previous frame's output copy. On coincide-mode hardware the
//! DPB layer itself is the output, with one spare layer for non-reference frames
//! that occupy no DPB slot.

use std::sync::Arc;

use ash::vk;

use super::codec::CodecProfile;
use super::device::Device;

/// The images one video session decodes into.
///
/// Recreated together with the session when the coded extent or slot count changes.
pub struct DecodeImages {
    device: Arc<Device>,
    dpb_view: vk::ImageView,
    dpb: super::alloc::Image,

    /// Output-image ring, empty on coincide-mode hardware.
    outputs: Vec<(super::alloc::Image, vk::ImageView)>,

    /// The ring entry the current frame decodes into, see [`Self::select_next_output`].
    current_output: usize,

    pub coded_extent: vk::Extent2D,
    pub coincide: bool,

    /// The first command buffer after creation transitions all layers out of
    /// `UNDEFINED`, see [`super::record`].
    pub needs_layout_init: bool,

    /// Layer count of the DPB image. In coincide mode this includes the spare
    /// output layer for non-reference frames after the `dpb_slots` slot layers.
    pub dpb_layers: u32,
}

impl DecodeImages {
    #[expect(unsafe_code)]
    pub fn new(
        device: Arc<Device>,
        profile: CodecProfile,
        coded_extent: vk::Extent2D,
        dpb_slots: u32,
        coincide: bool,
        output_ring_size: u32,
        decode_queue_family: u32,
        copy_queue_family: u32,
    ) -> Result<Self, vk::Result> {
        re_tracing::profile_function!();

        let concurrent_families = [decode_queue_family, copy_queue_family];
        let readback_usage = vk::ImageUsageFlags::TRANSFER_SRC;

        // Whichever image the readback copies from is used by both queue families:
        // CONCURRENT sharing sidesteps ownership transfers between them.
        let sharing = |copied_from: bool| {
            if copied_from && decode_queue_family != copy_queue_family {
                (vk::SharingMode::CONCURRENT, &concurrent_families[..])
            } else {
                (vk::SharingMode::EXCLUSIVE, &[][..])
            }
        };

        let image = |usage: vk::ImageUsageFlags, layers: u32, copied_from: bool| {
            let (sharing_mode, families) = sharing(copied_from);
            profile.with_profile_list(|profile_list| {
                let create_info = vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::G8_B8R8_2PLANE_420_UNORM)
                    .extent(vk::Extent3D {
                        width: coded_extent.width,
                        height: coded_extent.height,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(layers)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(usage)
                    .sharing_mode(sharing_mode)
                    .queue_family_indices(families)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .push_next(profile_list);
                super::alloc::Image::new(device.clone(), &create_info)
            })
        };

        let view = |image: &super::alloc::Image, view_type, layers| {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(image.raw)
                .view_type(view_type)
                .format(vk::Format::G8_B8R8_2PLANE_420_UNORM)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::COLOR)
                        .level_count(1)
                        .layer_count(layers),
                );
            // SAFETY: Plain creation, destroyed in drop.
            unsafe { device.raw.create_image_view(&create_info, None) }
        };

        let (dpb_layers, dpb_usage) = if coincide {
            (
                dpb_slots + 1,
                vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR
                    | vk::ImageUsageFlags::VIDEO_DECODE_DST_KHR
                    | readback_usage,
            )
        } else {
            (dpb_slots, vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR)
        };

        let dpb = image(dpb_usage, dpb_layers, coincide)?;
        let dpb_view = view(&dpb, vk::ImageViewType::TYPE_2D_ARRAY, dpb_layers)?;

        let mut outputs = Vec::new();
        if !coincide {
            #[expect(unsafe_code)]
            let cleanup = |outputs: &Vec<(super::alloc::Image, vk::ImageView)>| {
                // SAFETY: Nothing references the views yet.
                unsafe {
                    device.raw.destroy_image_view(dpb_view, None);
                    for (_, output_view) in outputs {
                        device.raw.destroy_image_view(*output_view, None);
                    }
                }
            };
            for _ in 0..output_ring_size.max(1) {
                let output = match image(
                    vk::ImageUsageFlags::VIDEO_DECODE_DST_KHR | readback_usage,
                    1,
                    true,
                ) {
                    Ok(output) => output,
                    Err(err) => {
                        cleanup(&outputs);
                        return Err(err);
                    }
                };
                match view(&output, vk::ImageViewType::TYPE_2D, 1) {
                    Ok(output_view) => outputs.push((output, output_view)),
                    Err(err) => {
                        cleanup(&outputs);
                        return Err(err);
                    }
                }
            }
        }

        Ok(Self {
            device,
            dpb_view,
            dpb,
            outputs,
            current_output: 0,
            coded_extent,
            coincide,
            needs_layout_init: true,
            dpb_layers,
        })
    }

    pub fn dpb_image(&self) -> vk::Image {
        self.dpb.raw
    }

    /// The picture resource of a DPB slot layer.
    pub fn dpb_resource(&self, layer: u32) -> vk::VideoPictureResourceInfoKHR<'static> {
        vk::VideoPictureResourceInfoKHR::default()
            .coded_extent(self.coded_extent)
            .base_array_layer(layer)
            .image_view_binding(self.dpb_view)
    }

    /// The layer of the DPB image a frame decodes into, `None` in distinct mode
    /// where output goes to the separate output image.
    pub fn dst_layer(&self, setup_slot: Option<u8>) -> Option<u32> {
        self.coincide.then(|| {
            // Non-reference frames occupy no slot and use the spare last layer.
            setup_slot.map_or(self.dpb_layers - 1, u32::from)
        })
    }

    /// Rotates to the next output-ring image in distinct mode, a no-op in
    /// coincide mode. Call once per frame before recording its decode.
    pub fn select_next_output(&mut self) {
        if !self.outputs.is_empty() {
            self.current_output = (self.current_output + 1) % self.outputs.len();
        }
    }

    /// The picture resource decode output is written to.
    pub fn dst_resource(&self, setup_slot: Option<u8>) -> vk::VideoPictureResourceInfoKHR<'static> {
        if let Some(layer) = self.dst_layer(setup_slot) {
            self.dpb_resource(layer)
        } else {
            let (_, output_view) = &self.outputs[self.current_output];
            vk::VideoPictureResourceInfoKHR::default()
                .coded_extent(self.coded_extent)
                .image_view_binding(*output_view)
        }
    }

    /// The image and layer the readback copies the decoded frame from.
    pub fn readback_source(&self, setup_slot: Option<u8>) -> (vk::Image, u32) {
        if let Some(layer) = self.dst_layer(setup_slot) {
            (self.dpb.raw, layer)
        } else {
            let (output, _) = &self.outputs[self.current_output];
            (output.raw, 0)
        }
    }

    /// The current output-ring image in distinct mode.
    pub fn current_output_image(&self) -> Option<vk::Image> {
        self.outputs
            .get(self.current_output)
            .map(|(image, _)| image.raw)
    }
}

impl Drop for DecodeImages {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        // The views go first, their images follow via field drop order.
        unsafe {
            self.device.raw.destroy_image_view(self.dpb_view, None);
            for (_, output_view) in &self.outputs {
                self.device.raw.destroy_image_view(*output_view, None);
            }
        }
    }
}
