use super::*;

/// Samples with equal timestamps complete in presentation order rather than submission order.
/// Each completion returns the metadata of its own sample.
#[test]
fn equal_timestamps_match_by_submission_id() {
    let mut pending = PendingFrameInfos::default();
    let sources = [0, 1, 2].map(|index| VideoSource::Span(re_span::Span::from_start_len(index, 1)));
    let ids: Vec<_> = [0, 2, 1]
        .into_iter()
        .enumerate()
        .map(|(index, frame_nr)| {
            pending.insert(PendingFrameInfo {
                is_sync: index == 0,
                frame_nr,
                source: sources[index],
                presentation_timestamp: Time(0),
                decode_timestamp: Time(i64::try_from(index).unwrap()),
                duration: None,
            })
        })
        .collect();

    for (frame_nr, index) in [0, 2, 1].into_iter().enumerate() {
        let info = pending.remove(ids[index]).unwrap();
        assert_eq!(info.frame_nr, frame_nr as FrameNumber);
        assert_eq!(info.source, sources[index]);
        assert_eq!(info.presentation_timestamp, Time(0));
        assert_eq!(info.decode_timestamp, Time(i64::try_from(index).unwrap()));
        assert_eq!(info.is_sync, index == 0);
    }
    assert!(pending.frames.is_empty());
}

/// Reset drops pending metadata and later submissions get distinct IDs.
#[test]
fn reset_drops_pending_metadata() {
    let mut pending = PendingFrameInfos::default();
    let info = || PendingFrameInfo {
        is_sync: true,
        frame_nr: 0,
        source: VideoSource::Span(re_span::Span::from_start_len(0, 1)),
        presentation_timestamp: Time(0),
        decode_timestamp: Time(0),
        duration: None,
    };
    let before = pending.insert(info());
    pending.clear();
    let after = pending.insert(info());
    assert_ne!(before, after);
    assert!(pending.remove(before).is_none());
    assert!(pending.remove(after).is_some());
}

fn video_device() -> (wgpu::Device, Arc<GpuVideoContext>) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    let (adapter, mut setup) = adapters
        .iter()
        .find_map(|adapter| {
            re_gpu_video::VideoDeviceSetup::request(adapter).map(|setup| (adapter, setup))
        })
        .expect("this test requires Vulkan Video H.264 decoding");
    let descriptor = wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_FORMAT_NV12,
        ..Default::default()
    };
    #[expect(unsafe_code)]
    // SAFETY: The hal device comes from this adapter with the descriptor used to
    // hand it to wgpu. The callback enables the video extensions and queues.
    let (device, _queue) = unsafe {
        let hal_adapter = adapter.as_hal::<wgpu::hal::api::Vulkan>().unwrap();
        let open_device = hal_adapter
            .open_with_callback(
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                Some(setup.create_device_callback()),
            )
            .unwrap();
        adapter
            .create_device_from_hal(open_device, &descriptor)
            .unwrap()
    };
    let context = setup.into_context(&device).unwrap();
    (device, context)
}

/// Several samples share a presentation timestamp while their frames await reordering.
/// Each output frame keeps the metadata of its own sample, including its source.
#[test]
#[ignore = "requires Vulkan Video H.264 decoding"]
fn samples_with_the_same_pts_keep_their_own_metadata() {
    let (device, context) = video_device();
    let mut decoder = GpuSyncDecoder {
        decoder: context.create_h264_decoder().unwrap(),
        input_format: InputFormat::AnnexB,
        annexb_buffer: Vec::new(),
        pending_frame_infos: PendingFrameInfos::default(),
        reorder_delay: Arc::new(AtomicUsize::new(0)),
    };
    let data = include_bytes!("../../../re_gpu_video/tests/assets/ipb.h264");
    let starts: Vec<_> = data
        .windows(4)
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == [0, 0, 1, 9]).then_some(index))
        .collect();
    assert!(starts.len() >= 4, "test asset needs access unit delimiters");
    let (sender, receiver) = crate::channel("duplicate GPU timestamps".to_owned());
    let sources = [0, 1, 2].map(|index| VideoSource::Span(re_span::Span::from_start_len(index, 1)));
    let should_stop = AtomicBool::new(false);

    for (index, frame_nr) in [0, 2, 1].into_iter().enumerate() {
        decoder.submit_chunk(
            &should_stop,
            Chunk {
                is_sync: index == 0,
                data: data[starts[index]..starts[index + 1]].to_vec(),
                sample_idx: index,
                frame_nr,
                source: sources[index],
                decode_timestamp: Time(i64::try_from(index).unwrap() - 3),
                presentation_timestamp: Time(0),
                duration: None,
            },
            &sender,
        );
        #[expect(unsafe_code)]
        // SAFETY: This test owns the decoder and submits from this thread only.
        // Each frame's GPU work completes before the next submission.
        unsafe {
            device
                .as_hal::<wgpu::hal::api::Vulkan>()
                .unwrap()
                .raw_device()
                .device_wait_idle()
                .unwrap();
        }
    }
    decoder.end_of_video(&sender);
    let mut infos = Vec::new();
    while let Ok(result) = receiver.try_recv() {
        infos.push(result.unwrap().info);
    }
    assert_eq!(
        infos.len(),
        3,
        "every submitted sample must produce a frame"
    );
    for (info, (frame_nr, source)) in
        std::iter::zip(&infos, [(0, sources[0]), (1, sources[2]), (2, sources[1])])
    {
        assert_eq!(info.presentation_timestamp, Time(0));
        assert_eq!(info.frame_nr, Some(frame_nr));
        assert_eq!(info.source, Some(source));
    }
}
