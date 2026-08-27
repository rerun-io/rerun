fn main() {
    cfg_aliases::cfg_aliases! {
        // Which backend modules get compiled in. Which backend is actually used is a runtime
        // decision keyed on the wgpu backend of the adapter, see `VideoDeviceSetup::request`.
        vulkan_video: { any(target_os = "windows", all(unix, not(target_vendor = "apple"))) },
        video_toolbox: { target_os = "macos" },
    }
}
