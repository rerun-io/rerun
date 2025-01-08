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
pub enum DeviceTier {
    /// Limited feature support as provided by WebGL and typically only by GLES2/OpenGL3(ish).
    ///
    /// Note that we do not distinguish between WebGL & native GL here,
    /// instead, we go with the lowest common denominator.
    /// In theory this path can also be hit on Vulkan & Metal drivers, but this is exceedingly rare.
    Gles = 0,

    /// Full support of WebGPU spec without additional feature requirements.
    ///
    /// Expecting to run either in a stable WebGPU implementation.
    /// I.e. either natively with Vulkan/Metal or in a browser with WebGPU support.
    FullWebGpuSupport = 1,
    // Run natively with Vulkan/Metal and require additional features.
    //HighEnd
}

impl DeviceTier {
    /// Whether the current device tier supports sampling from textures with a sample count higher than 1.
    pub fn support_sampling_msaa_texture(&self) -> bool {
        match self {
            Self::Gles => false,
            Self::FullWebGpuSupport => true,
        }
    }

    /// Whether the current device tier supports reading back depth textures.
    ///
    /// If this returns false, we first have to create a copy of the depth buffer by rendering depth to a different texture.
    pub fn support_depth_readback(&self) -> bool {
        match self {
            Self::Gles => false,
            Self::FullWebGpuSupport => true,
        }
    }

    pub fn support_bgra_textures(&self) -> bool {
        match self {
            // TODO(wgpu#3583): Incorrectly reported by wgpu right now.
            // GLES2 does not support BGRA textures!
            Self::Gles => false,
            Self::FullWebGpuSupport => true,
        }
    }

    /// Downlevel features required by the given tier.
    pub fn required_downlevel_capabilities(&self) -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: match self {
                Self::Gles => wgpu::DownlevelFlags::empty(),
                // Require fully WebGPU compliance for the native tier.
                Self::FullWebGpuSupport => wgpu::DownlevelFlags::all(),
            },
            limits: Default::default(), // unused so far both here and in wgpu as of writing.

            // Sm3 is missing a lot of features and even has an instruction count limit.
            // Sm4 is missing storage images and other minor features.
            // Sm5 is WebGPU compliant
            shader_model: wgpu::ShaderModel::Sm4,
        }
    }

    /// Required features for the given device tier.
    #[allow(clippy::unused_self)]
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
    #[error("Adapter does not support the minimum shader model required. Supported is {actual:?} but required is {required:?}.")]
    TooLowShaderModel {
        required: wgpu::ShaderModel,
        actual: wgpu::ShaderModel,
    },

    #[error("Adapter does not have all the required capability flags required. Supported are {actual:?} but required are {required:?}.")]
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
    pub tier: DeviceTier,

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
        let tier = if DeviceTier::FullWebGpuSupport
            .check_required_downlevel_capabilities(&downlevel_caps)
            .is_ok()
        {
            // We pass the WebGPU min-spec!
            DeviceTier::FullWebGpuSupport
        } else {
            DeviceTier::Gles
        };

        let backend_type = match adapter.get_info().backend {
            wgpu::Backend::Empty
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

        if caps.tier == DeviceTier::Gles {
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
            re_log::warn!("Running on a GPU/graphics driver with very limited abilitites. Consider updating your driver.");
        };

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
            memory_hints: Default::default(),
        }
    }
}

/// Backends that are officially supported by `re_renderer`.
///
/// Other backend might work as well, but lack of support isn't regarded as a bug.
pub fn supported_backends() -> wgpu::Backends {
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
        wgpu::util::backend_bits_from_env()
            .unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::GL)
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
        wgpu::Backend::Empty => {
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
