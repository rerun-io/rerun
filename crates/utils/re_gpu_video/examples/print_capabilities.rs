//! Prints GPU video decode capabilities for every adapter on this machine,
//! and smoke-tests device & context creation on the adapters that support decoding.
//!
//! ```sh
//! cargo run -p re_gpu_video --example print_capabilities
//! ```

use re_gpu_video::VideoDeviceSetup;

fn main() {
    re_log::setup_logging();

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    if adapters.is_empty() {
        println!("No graphics adapters found.");
        return;
    }

    for adapter in &adapters {
        let info = adapter.get_info();
        println!(
            "{} ({:?}, {:?}):",
            info.name, info.backend, info.device_type
        );

        let Some(mut setup) = VideoDeviceSetup::request(adapter) else {
            println!("  No video decode support.");
            continue;
        };
        for codec in [
            re_gpu_video::Codec::H264,
            re_gpu_video::Codec::H265,
            re_gpu_video::Codec::AV1,
        ] {
            match setup.capabilities(codec) {
                Some(capabilities) => println!("  {codec} decode support: {capabilities:#?}"),
                None => println!("  No {codec} decode support."),
            }
        }

        // Create a device the same way re_renderer does, to smoke-test the whole path.
        let descriptor = wgpu::DeviceDescriptor {
            label: Some("print_capabilities"),
            required_features: adapter
                .features()
                .intersection(wgpu::Features::TEXTURE_FORMAT_NV12),
            ..Default::default()
        };

        let device_result = if setup.needs_hal_device_creation() {
            #[expect(unsafe_code)]
            // SAFETY: Mirrors what `re_renderer::device::create_device` does:
            // the hal device is created from this adapter and handed straight to wgpu.
            unsafe {
                let hal_adapter = adapter
                    .as_hal::<wgpu::hal::api::Vulkan>()
                    .expect("probed adapter must be a Vulkan adapter");
                hal_adapter
                    .open_with_callback(
                        descriptor.required_features,
                        &descriptor.required_limits,
                        &descriptor.memory_hints,
                        Some(setup.create_device_callback()),
                    )
                    .map_err(|err| format!("hal device creation failed: {err}"))
                    .and_then(|open_device| {
                        adapter
                            .create_device_from_hal(open_device, &descriptor)
                            .map_err(|err| format!("wgpu device creation failed: {err}"))
                    })
            }
        } else {
            pollster::block_on(adapter.request_device(&descriptor))
                .map_err(|err| format!("wgpu device creation failed: {err}"))
        };

        match device_result {
            Ok((device, _queue)) => match setup.into_context(&device) {
                Ok(context) => {
                    println!(
                        "  Created a device and a {} context.",
                        context.backend_name()
                    );
                }
                Err(err) => println!("  Failed to create video context: {err}"),
            },
            Err(err) => println!("  {err}"),
        }
    }
}
