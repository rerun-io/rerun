/// Device tiers `re_renderer` distinguishes.
///
/// To reduce complexity, we rarely do fine-grained feature checks,
/// but instead support set of features, each a superset of the next.
///
/// Tiers are sorted from lowest to highest. Certain tiers may not be possible on a given machine/setup,
/// but choosing lower tiers is always possible.
/// Tiers may loosely relate to quality settings, but their primary function is an easier way to
/// do bundle feature *support* checks.
///
/// See also `global_bindings.wgsl`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceCapabilityTier {
    /// Limited feature support as provided by WebGL and some OpenGL drivers.
    ///
    /// On desktop this happens typically with GLES 2 & OpenGL 3.x, as well as some OpenGL 4.x drivers
    /// with lack of key rendering abilities.
    ///
    /// In theory this path can also be hit on Vulkan & Metal drivers, but this is exceedingly rare.
    Limited = 0,

    /// Full support of WebGPU spec without additional feature requirements.
    ///
    /// Expecting to run either in a stable WebGPU implementation.
    /// I.e. either natively with Vulkan/Metal or in a browser with WebGPU support.
    FullWebGpuSupport = 1,
    // Run natively with Vulkan/Metal and require additional features.
    //HighEnd
}

impl std::fmt::Display for DeviceCapabilityTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Limited => "limited",
            Self::FullWebGpuSupport => "full_webgpu_support",
        })
    }
}

impl DeviceCapabilityTier {
    /// Whether the current device tier supports sampling from textures with a sample count higher than 1.
    pub fn support_sampling_msaa_texture(&self) -> bool {
        match self {
            Self::Limited => false,
            Self::FullWebGpuSupport => true,
        }
    }

    /// Whether the current device tier supports reading back depth textures.
    ///
    /// If this returns false, we first have to create a copy of the depth buffer by rendering depth to a different texture.
    pub fn support_depth_readback(&self) -> bool {
        match self {
            Self::Limited => false,
            Self::FullWebGpuSupport => true,
        }
    }

    pub fn support_bgra_textures(&self) -> bool {
        match self {
            // TODO(wgpu#3583): Incorrectly reported by wgpu right now.
            // GLES2 does not support BGRA textures!
            Self::Limited => false,
            Self::FullWebGpuSupport => true,
        }
    }

    /// Downlevel features required by the given tier.
    pub fn required_downlevel_capabilities(&self) -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: match self {
                Self::Limited => wgpu::DownlevelFlags::empty(),
                // Require fully WebGPU compliance for the native tier.
                Self::FullWebGpuSupport => {
                    // Turn a blind eye on a few features that are missing as of writing in WSL even with latest Vulkan drivers.
                    // Pretend we still have full WebGPU support anyways.
                    wgpu::DownlevelFlags::compliant()
                        // Lacking `SURFACE_VIEW_FORMATS` means we can't set the format of views on surface textures
                        // (the result of `get_current_texture`).
                        // And the surface won't tell us which formats are supported.
                        // We avoid doing anything wonky with surfaces anyways, so we won't hit this.
                        .intersection(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS.complement())
                        // Lacking `FULL_DRAW_INDEX_UINT32` means that vertex indices above 2^24-1 are invalid.
                        // I.e. we can only draw with about 16.8mio vertices per mesh.
                        // Typically we don't reach this limit.
                        //
                        // This can happen if…
                        // * OpenGL: `GL_MAX_ELEMENT_INDEX` reports a value lower than `std::u32::MAX`
                        // * Vulkan: `VkPhysicalDeviceLimits::fullDrawIndexUint32` is false.
                        // The consequence of exceeding this limit seems to be undefined.
                        .intersection(wgpu::DownlevelFlags::FULL_DRAW_INDEX_UINT32.complement())
                }
            },
            limits: Default::default(), // unused so far both here and in wgpu as of writing.

            // Sm3 is missing a lot of features and even has an instruction count limit.
            // Sm4 is missing storage images and other minor features.
            // Sm5 is WebGPU compliant
            shader_model: wgpu::ShaderModel::Sm4,
        }
    }

    /// Required features for the given device tier.
    #[expect(clippy::unused_self)]
    pub fn features(&self) -> wgpu::Features {
        wgpu::Features::empty()
    }

    /// Check whether the given downlevel caps are sufficient for this tier.
    pub fn check_required_downlevel_capabilities(
        &self,
        downlevel_caps: &wgpu::DownlevelCapabilities,
    ) -> Result<(), InsufficientDeviceCapabilities> {
        let required_downlevel_caps_webgpu = self.required_downlevel_capabilities();

        if downlevel_caps.shader_model < required_downlevel_caps_webgpu.shader_model {
            Err(InsufficientDeviceCapabilities::TooLowShaderModel {
                required: required_downlevel_caps_webgpu.shader_model,
                actual: downlevel_caps.shader_model,
            })
        } else if !downlevel_caps
            .flags
            .contains(required_downlevel_caps_webgpu.flags)
        {
            Err(InsufficientDeviceCapabilities::MissingCapabilitiesFlags {
                required: required_downlevel_caps_webgpu.flags,
                actual: downlevel_caps.flags,
            })
        } else {
            Ok(())
        }
    }
}

/// Type of Wgpu backend.
///
/// Used in the rare cases where it's necessary to be aware of the api differences between
/// wgpu-core and webgpu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WgpuBackendType {
    /// Backend implemented via wgpu-core.
    ///
    /// This includes all native backends and WebGL.
    WgpuCore,

    /// Backend implemented by the browser's WebGPU javascript api.
    #[cfg(web)]
    WebGpu,
}

#[derive(thiserror::Error, Debug)]
pub enum InsufficientDeviceCapabilities {
    #[error(
        "Adapter does not support the minimum shader model required. Supported is {actual:?} but required is {required:?}."
    )]
    TooLowShaderModel {
        required: wgpu::ShaderModel,
        actual: wgpu::ShaderModel,
    },

    #[error(
        "Adapter does not have all the required capability flags required. Supported are {actual:?} but required are {required:?}."
    )]
    MissingCapabilitiesFlags {
        required: wgpu::DownlevelFlags,
        actual: wgpu::DownlevelFlags,
    },

    #[error("Adapter does not support drawing to texture format {format:?}")]
    CantDrawToTexture { format: wgpu::TextureFormat },
}

/// Capabilities of a given device.
///
/// Generally, this is a higher level interpretation of [`wgpu::Limits`] & [`wgpu::Features`].
///
/// We're trying to keep the number of fields in this struct to a minimum and associate
/// as many as possible capabilities with the device tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCaps {
    pub tier: DeviceCapabilityTier,

    /// Maximum texture dimension in pixels in both width and height.
    ///
    /// Since this has a direct effect on the image sizes & screen resolution a user can use, we always pick the highest possible.
    pub max_texture_dimension2d: u32,

    /// Maximum buffer size in bytes.
    ///
    /// Since this has a direct effect on how much data a user can wrangle on the gpu, we always pick the highest possible.
    pub max_buffer_size: u64,

    /// Wgpu backend type.
    ///
    /// Prefer using `tier` and other properties of this struct for distinguishing between abilities.
    /// This is useful for making wgpu-core/webgpu api path decisions.
    pub backend_type: WgpuBackendType,
}

impl DeviceCaps {
    /// Picks the highest possible tier for a given adapter, but doesn't validate that all the capabilities needed are there.
    ///
    /// This is really only needed for generating a device descriptor for [`Self::device_descriptor`].
    /// See also use of `egui_wgpu::WgpuSetup::CreateNew`
    pub fn from_adapter_without_validation(adapter: &wgpu::Adapter) -> Self {
        let downlevel_caps = adapter.get_downlevel_capabilities();

        // Note that non-GL backend doesn't automatically mean we support all downlevel flags.
        // (practically that's only the case for a handful of Vulkan/Metal devices and even so that's rare.
        // Practically all issues are with GL)
        let tier = if DeviceCapabilityTier::FullWebGpuSupport
            .check_required_downlevel_capabilities(&downlevel_caps)
            .is_ok()
        {
            // We pass the WebGPU min-spec!
            DeviceCapabilityTier::FullWebGpuSupport
        } else {
            DeviceCapabilityTier::Limited
        };

        let backend_type = match adapter.get_info().backend {
            wgpu::Backend::Noop
            | wgpu::Backend::Vulkan
            | wgpu::Backend::Metal
            | wgpu::Backend::Dx12
            | wgpu::Backend::Gl => WgpuBackendType::WgpuCore,
            wgpu::Backend::BrowserWebGpu => {
                #[cfg(web)]
                {
                    WgpuBackendType::WebGpu
                }
                #[cfg(not(web))]
                {
                    unreachable!("WebGPU backend is not supported on native platforms.")
                }
            }
        };
        let limits = adapter.limits();

        Self {
            tier,
            max_texture_dimension2d: limits.max_texture_dimension_2d,
            max_buffer_size: limits.max_buffer_size,
            backend_type,
        }
    }

    /// Picks the highest possible tier for a given adapter.
    ///
    /// Note that it is always possible to pick a lower tier!
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Result<Self, InsufficientDeviceCapabilities> {
        let caps = Self::from_adapter_without_validation(adapter);
        caps.tier
            .check_required_downlevel_capabilities(&adapter.get_downlevel_capabilities())?;

        if caps.tier == DeviceCapabilityTier::Limited {
            // Check texture format support. If `WEBGPU_TEXTURE_FORMAT_SUPPORT` is enabled, we're generally fine.
            // This is an implicit requirement for the WebGPU tier and above.
            if !adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT)
            {
                // Otherwise, make sure some basic formats are supported for drawing.
                // This is far from an exhaustive list, but it's a good sanity check for formats that may be missing.
                let formats_required_for_drawing = [
                    crate::ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                    // R32f has previously observed being missing on old OpenGL drivers and was fixed by updating the driver.
                    // https://github.com/rerun-io/rerun/issues/8466
                    // We use this as a fallback when depth readback is not support, but making this a general requirement
                    // seems wise as this is a great litmus test for potato drivers.
                    wgpu::TextureFormat::R32Float,
                    // The picking layer format is an integer texture. Might be slightly more challenging for some backends.
                    crate::PickingLayerProcessor::PICKING_LAYER_FORMAT,
                ];

                for format in formats_required_for_drawing {
                    if !adapter
                        .get_texture_format_features(format)
                        .allowed_usages
                        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                    {
                        return Err(InsufficientDeviceCapabilities::CantDrawToTexture { format });
                    }
                }
            }

            // Alright, this should still basically work.
            // This is really old though, so if we're not doing WebGL where this is kinda expected, let's issue a warning
            // in order to let the user know that they might be in trouble.
            //
            // In the long run we'd like WebGPU to be our minspec!
            // To learn more about the WebGPU minspec check:
            // * https://github.com/gpuweb/gpuweb/issues/1069
            // * https://www.w3.org/TR/webgpu/#adapter-capability-guarantees
            // * https://www.w3.org/TR/webgpu/#limits
            // This is roughly everything post 2014, so still VERY generous.
            //
            // It's much more likely we end up in here because of…
            // * older software rasterizer
            // * old/missing driver
            // * some VM/container setup with limited graphics capabilities.
            //
            // That's a lot of murky information, so let's keep the actual message crisp for now.
            #[cfg(not(web))]
            re_log::warn!(
                "Running on a GPU/graphics driver with very limited abilitites. Consider updating your driver."
            );
        }

        Ok(caps)
    }

    /// Wgpu limits required by the given device tier.
    pub fn limits(&self) -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: self.max_texture_dimension2d,
            max_buffer_size: self.max_buffer_size,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    /// Device descriptor compatible with the given device tier.
    pub fn device_descriptor(&self) -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some("re_renderer device"),
            required_features: self.tier.features(),
            required_limits: self.limits(),
            ..Default::default()
        }
    }
}

/// Returns an instance descriptor with settings preferred by `re_renderer`.
///
/// `re_renderer` should work fine with any instance descriptor, but those are the settings we generally assume.
pub fn instance_descriptor(force_backend: Option<&str>) -> wgpu::InstanceDescriptor {
    let supported_backends_str = wgpu::Instance::enabled_backend_features()
        .iter()
        .filter_map(|b| match b {
            wgpu::Backends::VULKAN => Some("vulkan"),
            wgpu::Backends::METAL => Some("metal"),
            wgpu::Backends::DX12 => Some("dx12"),
            wgpu::Backends::GL => Some("gl"),
            wgpu::Backends::BROWSER_WEBGPU => Some("webgpu"),

            #[expect(clippy::match_same_arms)]
            wgpu::Backends::NOOP => None, // Don't offer this even if it shows up (it shouldn't).
            _ => None, // Flag combinations shouldn't show up here.
        })
        .collect::<Vec<_>>()
        .join(", ");

    let backends = if let Some(force_backend) = force_backend {
        if let Some(backend) = parse_graphics_backend(force_backend) {
            if let Err(err) = validate_graphics_backend_applicability(backend) {
                re_log::error!(
                    "Failed to force rendering backend parsed from {force_backend:?}: {err} \
                    Supported on this platform are: {supported_backends_str}."
                );
                default_backends()
            } else {
                re_log::info!("Forcing graphics backend to {backend:?}.");
                backend.into()
            }
        } else {
            re_log::error!(
                "Failed to parse rendering backend string {force_backend:?}. \
                Supported on this platform are: {supported_backends_str}."
            );
            default_backends()
        }
    } else {
        default_backends()
    };

    wgpu::InstanceDescriptor {
        backends,
        flags: wgpu::InstanceFlags::default()
            // Allow adapters that aren't compliant with the backend they're implementing.
            // A concrete example of this is the latest Vulkan drivers on WSL which (as of writing)
            // advertise themselves as not being Vulkan compliant but work fine for the most part.
            //
            // In the future we might consider enabling this _only_ for WSL as this might otherwise
            // cause us to run with arbitrary development versions of drivers.
            // (then again, if a user has such a driver they likely *want* us to run with it anyways!)
            .union(wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        backend_options: wgpu::BackendOptions::default(),
    }
    // Allow manipulation of all options via environment variables.
    .with_env()
}

/// Returns an instance descriptor that is suitable for testing.
pub fn testing_instance_descriptor() -> wgpu::InstanceDescriptor {
    // We don't test on GL & DX12 right now (and don't want to do so by mistake!).
    // Several reasons for this:
    // * our CI is setup to draw with native Mac & lavapipe
    // * we generally prefer Vulkan over DX12 on Windows since it reduces the
    //   number of backends and wgpu's DX12 backend isn't as far along as of writing.
    // * we don't want to use the GL backend here since we regard it as a fallback only
    //   (TODO(andreas): Ideally we'd test that as well to check it is well-behaved,
    //   but for now we only want to have a look at the happy path)
    let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;

    let flags = (
        wgpu::InstanceFlags::ALLOW_UNDERLYING_NONCOMPLIANT_ADAPTER | wgpu::InstanceFlags::VALIDATION
        // TODO(andreas): GPU based validation layer sounds like a great idea,
        // but as of writing this makes tests crash on my Windows machine!
        // It looks like the crash is in the Vulkan/Nvidia driver, but further investigation is needed.
        // | wgpu::InstanceFlags::GPU_BASED_VALIDATION
    )
        .with_env(); // Allow overwriting flags via env vars.

    wgpu::InstanceDescriptor {
        backends,
        flags,
        ..instance_descriptor(None)
    }
}

/// Selects an adapter for testing, preferring software rendering if available.
///
/// Panics if no adapter was found.
#[cfg(native)]
pub fn select_testing_adapter(instance: &wgpu::Instance) -> wgpu::Adapter {
    let mut adapters = instance.enumerate_adapters(wgpu::Backends::all());
    assert!(!adapters.is_empty(), "No graphics adapter found!");

    re_log::info!("Found the following graphics adapters:");
    for adapter in &adapters {
        re_log::info!("* {}", crate::adapter_info_summary(&adapter.get_info()));
    }

    // Adapters are already sorted by preferred backend by wgpu, but let's be explicit.
    adapters.sort_by_key(|a| match a.get_info().backend {
        wgpu::Backend::Metal => 0,
        wgpu::Backend::Vulkan => 1,
        wgpu::Backend::Dx12 => 2,
        wgpu::Backend::Gl => 4,
        wgpu::Backend::BrowserWebGpu => 6,
        wgpu::Backend::Noop => 7,
    });

    // Prefer CPU adapters, otherwise if we can't, prefer discrete GPU over integrated GPU.
    adapters.sort_by_key(|a| match a.get_info().device_type {
        wgpu::DeviceType::Cpu => 0, // CPU is the best for our purposes!
        wgpu::DeviceType::DiscreteGpu => 1,
        wgpu::DeviceType::Other
        | wgpu::DeviceType::IntegratedGpu
        | wgpu::DeviceType::VirtualGpu => 2,
    });

    let adapter = adapters.remove(0);
    re_log::info!("Picked adapter: {:?}", adapter.get_info());

    adapter
}

/// Tries to select an adapter.
///
/// This is very similar to wgpu-core's implementation of `DynInstance::enumerate_adapters`.
pub fn select_adapter(
    adapters: &[wgpu::Adapter],
    enabled_backends: wgpu::Backends,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<wgpu::Adapter, String> {
    if adapters.is_empty() {
        return Err(format!(
            "No graphics adapter was found for the enabled graphics backends ({enabled_backends:?})"
        ));
    }

    let mut adapters = adapters.to_vec();

    // Filter out adapters that can't present to the given surface.
    if let Some(surface) = &surface {
        adapters.retain(|adapter| {
            let capabilities = surface.get_capabilities(adapter);
            if capabilities.formats.is_empty() {
                re_log::debug!(
                    "Adapter {:?} not compatible with the window's render surface.",
                    adapter.get_info()
                );
                false
            } else {
                true
            }
        });
        if adapters.is_empty() {
            return Err(
                "No graphics adapter was found that is compatible with the surface.".to_owned(),
            );
        }
    }

    re_log::debug!("Found the following viable graphics adapters:");
    for adapter in &adapters {
        re_log::debug!("* {}", crate::adapter_info_summary(&adapter.get_info()));
    }

    // Adapters are already sorted by preferred backend by wgpu, but let's be explicit.
    adapters.sort_by_key(|a| match a.get_info().backend {
        wgpu::Backend::Metal => 0,
        wgpu::Backend::Vulkan => 1,
        wgpu::Backend::Dx12 => 2,
        wgpu::Backend::Gl => 4,
        wgpu::Backend::BrowserWebGpu => 6,
        wgpu::Backend::Noop => 7,
    });

    // Prefer hardware adapters.
    adapters.sort_by_key(|a| match a.get_info().device_type {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::Other | wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Cpu => 3,
    });

    let adapter = adapters.remove(0);
    re_log::debug!("Picked adapter: {:?}", adapter.get_info());

    Ok(adapter)
}

/// Backends that are officially supported by `re_renderer`.
///
/// Other backend might work as well, but lack of support isn't regarded as a bug.
pub fn default_backends() -> wgpu::Backends {
    if cfg!(native) {
        // Native: Everything but DX12
        // * Wgpu's DX12 impl isn't in a great shape yet and there's now reason to add more variation
        //   when we can just use Vulkan
        //   So far, the main reason against it would be that some Windows VMs only provide DX12 drivers,
        //   observed with Parallels on Apple Silicon. In the future we might want to reconsider
        //   based on surface/presentation support which may be better with DX12.
        // * We'd like to exclude GL, but on Linux this can be a very useful fallback for users with
        //   with old hardware or bad/missing drivers. Wgpu automatically prefers Vulkan over GL when possible.
        //
        // For changing the backend we use standard wgpu env var, i.e. WGPU_BACKEND.
        wgpu::Backends::from_env()
            .unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::GL)
    } else if is_safari_browser() || is_firefox_browser() {
        // TODO(#10609): Fix WebGPU on Safari
        // TODO(#11009): Fix videos on WebGPU firefox
        wgpu::Backends::GL
    } else {
        wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU
    }
}

/// Generous parsing of a graphics backend string.
pub fn parse_graphics_backend(backend: &str) -> Option<wgpu::Backend> {
    match backend.to_lowercase().as_str() {
        // "vulcan" is a common typo that we just swallow. We know what you mean ;)
        "vulcan" | "vulkan" | "vk" => Some(wgpu::Backend::Vulkan),

        "metal" | "apple" | "mtl" => Some(wgpu::Backend::Metal),

        "dx12" | "dx" | "d3d" | "d3d12" | "directx" => Some(wgpu::Backend::Dx12),

        // We don't want to lie - e.g. `webgl1` should not work!
        // This means that `gles`/`gles3` stretches it a bit, but it's still close enough.
        // Similarly, we accept both `webgl` & `opengl` on each desktop & web.
        // This is a bit dubious but also too much hassle to forbid.
        "webgl2" | "webgl" | "opengl" | "gles" | "gles3" | "gl" => Some(wgpu::Backend::Gl),

        "browserwebgpu" | "webgpu" => Some(wgpu::Backend::BrowserWebGpu),

        _ => None,
    }
}

/// Validates that the given backend is applicable for the current build.
///
/// This is meant as a sanity check of first resort.
/// There are still many other reasons why a backend may not work on a given platform/build combination.
pub fn validate_graphics_backend_applicability(backend: wgpu::Backend) -> Result<(), &'static str> {
    match backend {
        wgpu::Backend::Noop => {
            // This should never happen.
            return Err("Cannot run with empty backend.");
        }
        wgpu::Backend::Vulkan => {
            // Through emulation and build configs Vulkan may work everywhere except the web.
            if cfg!(target_arch = "wasm32") {
                return Err("Can only run with WebGL or WebGPU on the web.");
            }
        }
        wgpu::Backend::Metal => {
            if cfg!(target_arch = "wasm32") {
                return Err("Can only run with WebGL or WebGPU on the web.");
            }
            if cfg!(target_os = "linux") || cfg!(target_os = "windows") {
                return Err("Cannot run with DX12 backend on Linux & Windows.");
            }
        }
        wgpu::Backend::Dx12 => {
            // We don't have DX12 enabled right now, but someone could.
            // TODO(wgpu#5166): But if we get this wrong we might crash.
            // TODO(wgpu#5167): And we also can't query the config.
            return Err("DX12 backend is currently not supported.");
        }
        wgpu::Backend::Gl => {
            // Using Angle Mac might actually run GL, but we don't enable this.
            // TODO(wgpu#5166): But if we get this wrong we might crash.
            // TODO(wgpu#5167): And we also can't query the config.
            if cfg!(target_os = "macos") {
                return Err("Cannot run with GL backend on Mac.");
            }
        }
        wgpu::Backend::BrowserWebGpu => {
            if !cfg!(target_arch = "wasm32") {
                return Err("Cannot run with WebGPU backend on native application.");
            }
        }
    }
    Ok(())
}

/// Are we running inside the Safari browser?
pub fn is_safari_browser() -> bool {
    #[cfg(target_arch = "wasm32")]
    fn is_safari_browser_inner() -> Option<bool> {
        use web_sys::wasm_bindgen::JsValue;
        let window = web_sys::window()?;
        Some(window.has_own_property(&JsValue::from("safari")))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn is_safari_browser_inner() -> Option<bool> {
        None
    }

    is_safari_browser_inner().unwrap_or(false)
}

/// Are we running inside the Firefox browser?
pub fn is_firefox_browser() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .is_some_and(|ua| ua.to_lowercase().contains("firefox"))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
