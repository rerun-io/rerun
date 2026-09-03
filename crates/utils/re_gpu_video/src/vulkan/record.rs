//! Records the per-frame decode and output-copy command buffers.
//!
//! The decode command buffer runs on the decode queue: layout transitions,
//! `vkCmdBeginVideoCodingKHR` (with a session reset on first use), the decode
//! operation wrapped in a result-status query, and the transition of the decode
//! output to `TRANSFER_SRC` for the copy. The output-copy command buffer runs on
//! the copy queue and copies the two NV12 planes of the display region either
//! into a host-visible buffer (the CPU debugging path) or into a new NV12
//! image handed to wgpu (see [`super::output`]).

use ash::vk;

use super::device::Device;
use super::dpb::DecodeImages;
use super::h264::DecodeInfo;
use super::session::{SessionParameters, VideoSession};

/// Everything [`record_decode`] needs besides the session objects.
pub struct FrameDecode<'a> {
    pub info: &'a DecodeInfo,

    /// Bitstream buffer holding the slice NALs, each prefixed with a 3-byte start code.
    pub bitstream_buffer: vk::Buffer,

    /// Used range of the bitstream buffer, aligned to the device's
    /// bitstream size alignment.
    pub bitstream_size: u64,

    /// Offsets of the slices (at their start codes) within the buffer.
    pub slice_offsets: &'a [u32],

    /// The result-status query this decode writes: the frame's resource-slot index.
    pub query_index: u32,
}

/// The image the output copy reads the decoded frame from, left in
/// `TRANSFER_SRC_OPTIMAL` by [`record_decode`].
pub struct CopySource {
    pub image: vk::Image,
    pub layer: u32,

    /// The source is also a DPB slot (coincide-mode hardware) and must be
    /// transitioned back to the DPB layout after the copy.
    pub restore_dpb_layout: bool,

    /// Top-left corner of the display region within the coded image, in luma texels.
    pub crop_offset: [i32; 2],

    /// Display size in luma texels.
    pub display: [u32; 2],
}

/// Where the readback copy puts the two planes of the decoded frame.
pub struct Readback {
    pub buffer: vk::Buffer,

    /// Byte offset of the chroma plane within the buffer (the luma plane is at 0).
    pub uv_buffer_offset: u64,
}

fn image_barrier(
    image: vk::Image,
    base_layer: u32,
    layer_count: u32,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src: (vk::PipelineStageFlags2, vk::AccessFlags2),
    dst: (vk::PipelineStageFlags2, vk::AccessFlags2),
) -> vk::ImageMemoryBarrier2<'static> {
    vk::ImageMemoryBarrier2::default()
        .src_stage_mask(src.0)
        .src_access_mask(src.1)
        .dst_stage_mask(dst.0)
        .dst_access_mask(dst.1)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .image(image)
        .subresource_range(
            vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .base_array_layer(base_layer)
                .layer_count(layer_count)
                .level_count(1),
        )
}

#[expect(unsafe_code)]
fn pipeline_barrier(
    device: &Device,
    cmd: vk::CommandBuffer,
    barriers: &[vk::ImageMemoryBarrier2<'_>],
) {
    let dependency = vk::DependencyInfo::default().image_memory_barriers(barriers);
    // SAFETY: The command buffer is in recording state.
    unsafe {
        device.raw.cmd_pipeline_barrier2(cmd, &dependency);
    }
}

fn std_reference_info(
    frame_num: u16,
    top_field_order_cnt: i32,
    bottom_field_order_cnt: i32,
    is_long_term: bool,
) -> vk::native::StdVideoDecodeH264ReferenceInfo {
    let mut flags = vk::native::StdVideoDecodeH264ReferenceInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: vk::native::__BindgenBitfieldUnit::new([0; 1]),
        __bindgen_padding_0: [0; 3],
    };
    flags.set_used_for_long_term_reference(is_long_term.into());

    vk::native::StdVideoDecodeH264ReferenceInfo {
        flags,
        FrameNum: frame_num,
        reserved: 0,
        PicOrderCnt: [top_field_order_cnt, bottom_field_order_cnt],
    }
}

fn std_picture_info(info: &DecodeInfo) -> vk::native::StdVideoDecodeH264PictureInfo {
    let mut flags = vk::native::StdVideoDecodeH264PictureInfoFlags {
        _bitfield_align_1: [],
        _bitfield_1: vk::native::__BindgenBitfieldUnit::new([0; 1]),
        __bindgen_padding_0: [0; 3],
    };
    // Interlaced content never gets here, so the field flags stay zero.
    flags.set_is_intra(info.is_intra.into());
    flags.set_IdrPicFlag(info.is_idr.into());
    flags.set_is_reference(info.setup_slot.is_some().into());

    vk::native::StdVideoDecodeH264PictureInfo {
        flags,
        seq_parameter_set_id: info.sps_id,
        pic_parameter_set_id: info.pps_id,
        reserved1: 0,
        reserved2: 0,
        frame_num: info.frame_num,
        idr_pic_id: info.idr_pic_id,
        PicOrderCnt: [info.top_field_order_cnt, info.bottom_field_order_cnt],
    }
}

/// Records the whole decode of one frame into `cmd` (already in recording state).
///
/// Leaves the decode output in `TRANSFER_SRC_OPTIMAL` for the readback copy.
#[expect(unsafe_code)]
pub fn record_decode(
    device: &Device,
    cmd: vk::CommandBuffer,
    session: &mut VideoSession,
    parameters: &SessionParameters,
    images: &mut DecodeImages,
    frame: &FrameDecode<'_>,
) {
    re_tracing::profile_function!();

    let info = frame.info;

    // Result-status queries are reset outside of the video coding scope.
    if let Some(query_pool) = session.query_pool {
        // SAFETY: The command buffer is in recording state.
        unsafe {
            device
                .raw
                .cmd_reset_query_pool(cmd, query_pool, frame.query_index, 1);
        }
    }

    let decode_stage = (
        vk::PipelineStageFlags2::VIDEO_DECODE_KHR,
        vk::AccessFlags2::VIDEO_DECODE_READ_KHR | vk::AccessFlags2::VIDEO_DECODE_WRITE_KHR,
    );

    let mut barriers = Vec::new();
    if images.needs_layout_init {
        images.needs_layout_init = false;
        barriers.push(image_barrier(
            images.dpb_image(),
            0,
            images.dpb_layers,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::VIDEO_DECODE_DPB_KHR,
            (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
            decode_stage,
        ));
    }
    if let Some(output) = images.current_output_image() {
        // This ring image's previous content is dead, discard it.
        barriers.push(image_barrier(
            output,
            0,
            1,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::VIDEO_DECODE_DST_KHR,
            (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
            decode_stage,
        ));
    }
    // Consecutive decode submissions overlap on the queue with no other
    // synchronization between them: order this frame's DPB reads and writes
    // after the previous frame's.
    let decode_ordering = [vk::MemoryBarrier2::default()
        .src_stage_mask(decode_stage.0)
        .src_access_mask(decode_stage.1)
        .dst_stage_mask(decode_stage.0)
        .dst_access_mask(decode_stage.1)];
    let dependency = vk::DependencyInfo::default()
        .memory_barriers(&decode_ordering)
        .image_memory_barriers(&barriers);
    // SAFETY: The command buffer is in recording state.
    unsafe {
        device.raw.cmd_pipeline_barrier2(cmd, &dependency);
    }

    // The reference slots of this decode. The resources and H.264 slot infos must
    // stay in place while the slot infos point at them, hence the separate vecs.
    let std_refs: Vec<vk::native::StdVideoDecodeH264ReferenceInfo> = info
        .references
        .iter()
        .map(|reference| {
            std_reference_info(
                reference.frame_num,
                reference.top_field_order_cnt,
                reference.bottom_field_order_cnt,
                reference.is_long_term,
            )
        })
        .collect();
    let resources: Vec<vk::VideoPictureResourceInfoKHR<'_>> = info
        .references
        .iter()
        .map(|reference| images.dpb_resource(u32::from(reference.slot)))
        .collect();
    let mut slot_infos: Vec<vk::VideoDecodeH264DpbSlotInfoKHR<'_>> = std_refs
        .iter()
        .map(|std_ref| vk::VideoDecodeH264DpbSlotInfoKHR::default().std_reference_info(std_ref))
        .collect();
    let reference_slots: Vec<vk::VideoReferenceSlotInfoKHR<'_>> =
        itertools::izip!(&info.references, &resources, slot_infos.iter_mut())
            .map(|(reference, resource, slot_info)| {
                vk::VideoReferenceSlotInfoKHR::default()
                    .slot_index(i32::from(reference.slot))
                    .picture_resource(resource)
                    .push_next(slot_info)
            })
            .collect();

    // The DPB slot this frame activates, with the frame's own reference metadata.
    // Long-term frames are addressed by their long-term index from then on.
    // Non-reference frames get the scratch slot: they activate a slot nothing
    // references, because RADV crashes when the setup slot is left out.
    let activated_slot = info.activated_slot();
    let setup_resource = activated_slot.map(|slot| images.dpb_resource(u32::from(slot)));
    let setup_std_ref = std_reference_info(
        info.long_term_frame_idx.unwrap_or(info.frame_num),
        info.top_field_order_cnt,
        info.bottom_field_order_cnt,
        info.long_term_frame_idx.is_some(),
    );
    let mut setup_slot_h264 =
        vk::VideoDecodeH264DpbSlotInfoKHR::default().std_reference_info(&setup_std_ref);
    let setup_slot = activated_slot.map(|slot| {
        vk::VideoReferenceSlotInfoKHR::default()
            .slot_index(i32::from(slot))
            .picture_resource(setup_resource.as_ref().expect("set together"))
            .push_next(&mut setup_slot_h264)
    });

    // Everything used within the video coding scope must be bound at its begin:
    // the active references with their slot indices, plus (index -1, not yet a slot)
    // the resource the setup slot activates, or in coincide mode the spare output
    // layer a non-reference frame decodes into.
    let mut begin_slots = reference_slots.clone();
    let extra_resource = match (&setup_resource, images.dst_layer(None)) {
        (Some(resource), _) => Some(*resource),
        (None, Some(spare_layer)) if images.coincide => Some(images.dpb_resource(spare_layer)),
        _ => None,
    };
    if let Some(resource) = &extra_resource {
        begin_slots.push(
            vk::VideoReferenceSlotInfoKHR::default()
                .slot_index(-1)
                .picture_resource(resource),
        );
    }

    let begin_info = vk::VideoBeginCodingInfoKHR::default()
        .video_session(session.raw)
        .video_session_parameters(parameters.raw)
        .reference_slots(&begin_slots);

    // SAFETY: The command buffer is in recording state and every struct chained into
    // the infos outlives the recording calls.
    unsafe {
        (device.video_queue_fns.fp().cmd_begin_video_coding_khr)(cmd, &raw const begin_info);

        if session.needs_reset {
            session.needs_reset = false;
            let control_info = vk::VideoCodingControlInfoKHR::default()
                .flags(vk::VideoCodingControlFlagsKHR::RESET);
            (device.video_queue_fns.fp().cmd_control_video_coding_khr)(
                cmd,
                &raw const control_info,
            );
        }

        if let Some(query_pool) = session.query_pool {
            device.raw.cmd_begin_query(
                cmd,
                query_pool,
                frame.query_index,
                vk::QueryControlFlags::empty(),
            );
        }

        let std_pic = std_picture_info(info);
        let mut h264_picture_info = vk::VideoDecodeH264PictureInfoKHR::default()
            .std_picture_info(&std_pic)
            .slice_offsets(frame.slice_offsets);
        let mut decode_info = vk::VideoDecodeInfoKHR::default()
            .src_buffer(frame.bitstream_buffer)
            .src_buffer_offset(0)
            .src_buffer_range(frame.bitstream_size)
            .dst_picture_resource(images.dst_resource(activated_slot))
            .reference_slots(&reference_slots)
            .push_next(&mut h264_picture_info);
        if let Some(setup_slot) = &setup_slot {
            decode_info = decode_info.setup_reference_slot(setup_slot);
        }
        (device.video_decode_fns.fp().cmd_decode_video_khr)(cmd, &raw const decode_info);

        if let Some(query_pool) = session.query_pool {
            device.raw.cmd_end_query(cmd, query_pool, frame.query_index);
        }

        let end_info = vk::VideoEndCodingInfoKHR::default();
        (device.video_queue_fns.fp().cmd_end_video_coding_khr)(cmd, &raw const end_info);
    }

    // Hand the decode output to the copy queue. Visibility across the queues comes
    // from the timeline semaphore, the barrier only performs the layout transition.
    let (readback_image, readback_layer) = images.readback_source(activated_slot);
    let old_layout = if images.coincide {
        vk::ImageLayout::VIDEO_DECODE_DPB_KHR
    } else {
        vk::ImageLayout::VIDEO_DECODE_DST_KHR
    };
    pipeline_barrier(
        device,
        cmd,
        &[image_barrier(
            readback_image,
            readback_layer,
            1,
            old_layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            decode_stage,
            (
                vk::PipelineStageFlags2::ALL_COMMANDS,
                vk::AccessFlags2::NONE,
            ),
        )],
    );
}

/// Records the copy of the decoded frame's display region into the readback buffer,
/// cropping the coded image down to the display size.
#[expect(unsafe_code)]
pub fn record_output_to_buffer(
    device: &Device,
    cmd: vk::CommandBuffer,
    source: &CopySource,
    readback: &Readback,
) {
    re_tracing::profile_function!();

    let subresource = |aspect| {
        vk::ImageSubresourceLayers::default()
            .aspect_mask(aspect)
            .base_array_layer(source.layer)
            .layer_count(1)
    };

    let [crop_x, crop_y] = source.crop_offset;
    let [display_width, display_height] = source.display;

    // Plane offsets and extents are in each plane's own texel coordinates,
    // half resolution for the chroma plane. 4:2:0 crop offsets are always even.
    let regions = [
        vk::BufferImageCopy2::default()
            .buffer_offset(0)
            .image_subresource(subresource(vk::ImageAspectFlags::PLANE_0))
            .image_offset(vk::Offset3D {
                x: crop_x,
                y: crop_y,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width: display_width,
                height: display_height,
                depth: 1,
            }),
        vk::BufferImageCopy2::default()
            .buffer_offset(readback.uv_buffer_offset)
            .image_subresource(subresource(vk::ImageAspectFlags::PLANE_1))
            .image_offset(vk::Offset3D {
                x: crop_x / 2,
                y: crop_y / 2,
                z: 0,
            })
            .image_extent(vk::Extent3D {
                width: display_width.div_ceil(2),
                height: display_height.div_ceil(2),
                depth: 1,
            }),
    ];
    let copy_info = vk::CopyImageToBufferInfo2::default()
        .src_image(source.image)
        .src_image_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .dst_buffer(readback.buffer)
        .regions(&regions);

    // SAFETY: The command buffer is in recording state.
    unsafe {
        device.raw.cmd_copy_image_to_buffer2(cmd, &copy_info);
    }

    // Make the copy visible to the host read after the semaphore wait.
    let host_read_barriers = [vk::MemoryBarrier2::default()
        .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
        .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
        .dst_stage_mask(vk::PipelineStageFlags2::HOST)
        .dst_access_mask(vk::AccessFlags2::HOST_READ)];
    let restore_barriers = source_restore_barriers(source);
    let dependency = vk::DependencyInfo::default()
        .image_memory_barriers(&restore_barriers)
        .memory_barriers(&host_read_barriers);
    // SAFETY: The command buffer is in recording state.
    unsafe {
        device.raw.cmd_pipeline_barrier2(cmd, &dependency);
    }
}

/// Records the copy of the decoded frame's display region into a new NV12 image
/// that is handed to wgpu, cropping the coded image down to `dst_extent`.
///
/// `dst_extent` is the display size rounded up to even (NV12 images need even sizes),
/// the destination image's own extent. The image's previous content is discarded, and
/// it is left in `TRANSFER_DST_OPTIMAL`: the layout matching the `COPY_DST` state it
/// is handed to wgpu in.
#[expect(unsafe_code)]
pub fn record_output_to_image(
    device: &Device,
    cmd: vk::CommandBuffer,
    source: &CopySource,
    dst_image: vk::Image,
    dst_extent: vk::Extent2D,
) {
    re_tracing::profile_function!();

    pipeline_barrier(
        device,
        cmd,
        &[image_barrier(
            dst_image,
            0,
            1,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            (vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE),
            (
                vk::PipelineStageFlags2::TRANSFER,
                vk::AccessFlags2::TRANSFER_WRITE,
            ),
        )],
    );

    let subresource = |aspect, layer| {
        vk::ImageSubresourceLayers::default()
            .aspect_mask(aspect)
            .base_array_layer(layer)
            .layer_count(1)
    };
    let [crop_x, crop_y] = source.crop_offset;

    // Plane offsets and extents are in each plane's own texel coordinates,
    // half resolution for the chroma plane. 4:2:0 crop offsets are always even,
    // and the even destination extent never reaches past the coded size:
    // an odd display size means at least one texel of crop on the far side.
    let regions = [
        vk::ImageCopy2::default()
            .src_subresource(subresource(vk::ImageAspectFlags::PLANE_0, source.layer))
            .src_offset(vk::Offset3D {
                x: crop_x,
                y: crop_y,
                z: 0,
            })
            .dst_subresource(subresource(vk::ImageAspectFlags::PLANE_0, 0))
            .extent(vk::Extent3D {
                width: dst_extent.width,
                height: dst_extent.height,
                depth: 1,
            }),
        vk::ImageCopy2::default()
            .src_subresource(subresource(vk::ImageAspectFlags::PLANE_1, source.layer))
            .src_offset(vk::Offset3D {
                x: crop_x / 2,
                y: crop_y / 2,
                z: 0,
            })
            .dst_subresource(subresource(vk::ImageAspectFlags::PLANE_1, 0))
            .extent(vk::Extent3D {
                width: dst_extent.width / 2,
                height: dst_extent.height / 2,
                depth: 1,
            }),
    ];
    let copy_info = vk::CopyImageInfo2::default()
        .src_image(source.image)
        .src_image_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .dst_image(dst_image)
        .dst_image_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
        .regions(&regions);

    // SAFETY: The command buffer is in recording state.
    unsafe {
        device.raw.cmd_copy_image2(cmd, &copy_info);
    }

    // No barrier for the destination: the image only goes to wgpu once the host
    // observed the copy's semaphore value, which orders it against wgpu's
    // submissions, and wgpu's own transition out of `COPY_DST` makes the
    // transfer write visible there.
    let barriers = source_restore_barriers(source);
    if !barriers.is_empty() {
        pipeline_barrier(device, cmd, &barriers);
    }
}

/// In coincide mode the copy source is also a DPB slot and stays a valid
/// reference picture, so its layout is restored after the copy read.
fn source_restore_barriers(source: &CopySource) -> Vec<vk::ImageMemoryBarrier2<'static>> {
    if !source.restore_dpb_layout {
        return Vec::new();
    }
    vec![image_barrier(
        source.image,
        source.layer,
        1,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        vk::ImageLayout::VIDEO_DECODE_DPB_KHR,
        (
            vk::PipelineStageFlags2::TRANSFER,
            vk::AccessFlags2::TRANSFER_READ,
        ),
        (
            vk::PipelineStageFlags2::ALL_COMMANDS,
            vk::AccessFlags2::NONE,
        ),
    )]
}
