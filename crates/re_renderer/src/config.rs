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
    /// Limited feature support as provided by WebGL and native GLES2/OpenGL3(ish).
    ///
    /// Note that we do not distinguish between WebGL & native GL here,
    /// instead, we go with the lowest common denominator.
    Gles = 0,

    /// Full support of WebGPU spec without additional feature requirements.
    ///
    /// Expecting to run either in a stable WebGPU implementation.
    /// I.e. either natively with Vulkan/Metal or in a browser with WebGPU support.
    FullWebGpuSupport = 1,
    // Run natively with Vulkan/Metal and require additional features.
    //HighEnd
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
    #[error("Adapter does not support the minimum shader model required. Supported is {actual:?} but required is {required:?}")]
    TooLowShaderModel {
        required: wgpu::ShaderModel,
        actual: wgpu::ShaderModel,
    },

    #[error("Adapter does not have all the required capability flags required. Supported are {actual:?} but required are {required:?}")]
    MissingCapabilitiesFlags {
        required: wgpu::DownlevelFlags,
        actual: wgpu::DownlevelFlags,
    },
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

    /// Wgpu backend type.
    ///
    /// Prefer using `tier` and other properties of this struct for distinguishing between abilities.
    /// This is useful for making wgpu-core/webgpu api path decisions.
    pub backend_type: WgpuBackendType,
}

impl DeviceCaps {
    /// Whether the current device tier supports sampling from textures with a sample count higher than 1.
    pub fn support_sampling_msaa_texture(&self) -> bool {
        match self.tier {
            DeviceTier::Gles => false,
            DeviceTier::FullWebGpuSupport => true,
        }
    }

    /// Whether the current device tier supports sampling from textures with a sample count higher than 1.
    pub fn support_depth_readback(&self) -> bool {
        match self.tier {
            DeviceTier::Gles => false,
            DeviceTier::FullWebGpuSupport => true,
        }
    }

    /// Picks the highest possible tier for a given adapter.
    ///
    /// Note that it is always possible to pick a lower tier!
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let backend = adapter.get_info().backend;

        let tier = match backend {
            wgpu::Backend::Vulkan
            | wgpu::Backend::Metal
            | wgpu::Backend::Dx12
            | wgpu::Backend::BrowserWebGpu => DeviceTier::FullWebGpuSupport,

            wgpu::Backend::Gl | wgpu::Backend::Empty => DeviceTier::Gles,
        };

        let backend_type = match backend {
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

        Self {
            tier,
            max_texture_dimension2d: adapter.limits().max_texture_dimension_2d,
            backend_type,
        }
    }

    /// Wgpu limits required by the given device tier.
    pub fn limits(&self) -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: self.max_texture_dimension2d,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    /// Required features for the given device tier.
    #[allow(clippy::unused_self)]
    pub fn features(&self) -> wgpu::Features {
        wgpu::Features::empty()
    }

    /// Device descriptor compatible with the given device tier.
    pub fn device_descriptor(&self) -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some("re_renderer device"),
            required_features: self.features(),
            required_limits: self.limits(),
        }
    }

    /// Downlevel features required by the given tier.
    pub fn required_downlevel_capabilities(&self) -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: match self.tier {
                DeviceTier::Gles => wgpu::DownlevelFlags::empty(),
                // Require fully WebGPU compliance for the native tier.
                DeviceTier::FullWebGpuSupport => wgpu::DownlevelFlags::all(),
            },
            limits: Default::default(), // unused so far both here and in wgpu
            shader_model: wgpu::ShaderModel::Sm4,
        }
    }

    /// Checks if passed downlevel capabilities support the given device tier.
    pub fn check_downlevel_capabilities(
        &self,
        capabilities: &wgpu::DownlevelCapabilities,
    ) -> Result<(), InsufficientDeviceCapabilities> {
        let wgpu::DownlevelCapabilities {
            flags,
            limits: _,
            shader_model,
        } = self.required_downlevel_capabilities();

        if capabilities.shader_model < shader_model {
            Err(InsufficientDeviceCapabilities::TooLowShaderModel {
                required: shader_model,
                actual: capabilities.shader_model,
            })
        } else if !capabilities.flags.contains(flags) {
            Err(InsufficientDeviceCapabilities::MissingCapabilitiesFlags {
                required: flags,
                actual: capabilities.flags,
            })
        } else {
            Ok(())
        }
    }
}

/// Startup configuration for a [`crate::RenderContext`]
///
/// Contains any kind of configuration that doesn't change for the entire lifetime of a [`crate::RenderContext`].
/// (flipside, if we do want to change any of these, the [`crate::RenderContext`] needs to be re-created)
pub struct RenderContextConfig {
    /// The color format used by the eframe output buffer.
    pub output_format_color: wgpu::TextureFormat,

    /// Hardware capabilities of the device.
    pub device_caps: DeviceCaps,
}

/// Backends that are officially supported by `re_renderer`.
///
/// Other backend might work as well, but lack of support isn't regarded as a bug.
pub fn supported_backends() -> wgpu::Backends {
    if cfg!(native) {
        // Native.
        // Only use Vulkan & Metal unless explicitly told so since this reduces surfaces and thus surprises.
        //
        // Bunch of cases where it's still useful to switch though:
        // * Some Windows VMs only provide DX12 drivers, observed with Parallels on Apple Silicon
        // * May run into Linux issues that warrant trying out the GL backend.
        //
        // For changing the backend we use standard wgpu env var, i.e. WGPU_BACKEND.
        wgpu::util::backend_bits_from_env()
            .unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::METAL)
    } else {
        wgpu::Backends::GL | wgpu::Backends::BROWSER_WEBGPU
    }
}
