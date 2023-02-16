/// Hardware tiers `re_renderer` distinguishes.
///
/// To reduce complexity, we don't do fine-grained feature checks,
/// but instead support set of features, each a superset of the next.
#[derive(Clone, Copy, Debug)]
pub enum HardwareTier {
    /// Maintains strict WebGL capability.
    Web,
    // Run natively with Vulkan/Metal but don't demand anything that isn't widely available.
    //Native,
    // Run natively with Vulkan/Metal and require additional features.
    //HighEnd
}

impl HardwareTier {
    /// Wgpu limits required by the given hardware tier.
    pub fn limits(self) -> wgpu::Limits {
        wgpu::Limits {
            // In any scenario require high texture resolution to facilitate rendering into large surfaces
            // (important for 4k screens and beyond)
            // 8192 is widely supported by now.
            max_texture_dimension_2d: 8192,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    /// Required features for the given hardware tier.
    pub fn features(self) -> wgpu::Features {
        wgpu::Features::empty()
    }

    /// Device descriptor compatible with the given hardware tier.
    pub fn device_descriptor(self) -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            label: Some("re_renderer device"),
            features: self.features(),
            limits: self.limits(),
        }
    }

    /// Downlevel features required by the given tier.
    pub fn required_downlevel_capabilities(self) -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: wgpu::DownlevelFlags::empty(),
            limits: Default::default(), // unused so far both here and in wgpu
            shader_model: wgpu::ShaderModel::Sm4,
        }
    }

    /// Checks if passed downlevel capabilities support the given hardware tier.
    pub fn check_downlevel_capabilities(
        self,
        downlevel_capabilities: &wgpu::DownlevelCapabilities,
    ) -> anyhow::Result<()> {
        let required_downlevel_capabilities = self.required_downlevel_capabilities();
        anyhow::ensure!(
            downlevel_capabilities.shader_model >= required_downlevel_capabilities.shader_model,
            "Adapter does not support the minimum shader model required to run re_renderer at the {:?} tier: {:?}",
            self,
            required_downlevel_capabilities.shader_model
        );
        anyhow::ensure!(
            downlevel_capabilities
                .flags
                .contains(required_downlevel_capabilities.flags),
            "Adapter does not support the downlevel capabilities required to run re_renderer at the {:?} tier: {:?}",
            self,
            required_downlevel_capabilities.flags - downlevel_capabilities.flags
        );

        Ok(())
    }
}

/// Startup configuration for a [`crate::RenderContext`]
///
/// Contains any kind of configuration that doesn't change for the entire lifetime of a [`crate::RenderContext`].
/// (flipside, if we do want to change any of these, the [`crate::RenderContext`] needs to be re-created)
pub struct RenderContextConfig {
    /// The color format used by the eframe output buffer.
    pub output_format_color: wgpu::TextureFormat,

    /// The targeted hardware tier.
    ///
    /// Passed devices are expected to fulfill all restrictions on the provided tier.
    pub hardware_tier: HardwareTier,
}

/// Backends that are officially supported by `re_renderer`.
///
/// Other backend might work as well, but lack of support isn't regarded as a bug.
pub fn supported_backends() -> wgpu::Backends {
    // Native.
    #[cfg(not(target_arch = "wasm32"))]
    {
        // We primarily test Vulkan & Metal!
        // Ideally we'd only have these two in order to keep our variance low.
        wgpu::Backends::VULKAN | wgpu::Backends::METAL |

        // DX12 is added since, as of writing some Windows VMs provide DX12 but no Vulkan drivers.
        // (this has been observed with Parallels on Apple Silicon)
        // Having this in means that wgpu will pick DX12 over Vulkan.
        wgpu::Backends::DX12 |

        // Add GL as a fallback to try when there is something wrong with Vulkan.
        wgpu::Backends::GL
    }
    // Web - we support only WebGL right now, WebGPU should work but hasn't been tested.
    #[cfg(target_arch = "wasm32")]
    {
        wgpu::Backends::GL
    }
}
