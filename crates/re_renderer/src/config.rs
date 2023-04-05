/// Hardware tiers `re_renderer` distinguishes.
///
/// To reduce complexity, we don't do fine-grained feature checks,
/// but instead support set of features, each a superset of the next.
#[derive(Clone, Copy, Debug)]
pub enum HardwareTier {
    /// For WebGL and native OpenGL. Maintains strict WebGL capability.
    Web,

    /// Run natively with Vulkan/Metal but don't demand anything that isn't widely available.
    Native,
    // Run natively with Vulkan/Metal and require additional features.
    //HighEnd
}

impl HardwareTier {
    /// Whether the current hardware tier supports sampling from textures with a sample count higher than 1.
    pub fn support_sampling_msaa_texture(&self) -> bool {
        match self {
            HardwareTier::Web => false,
            HardwareTier::Native => true,
        }
    }

    /// Whether the current hardware tier supports sampling from textures with a sample count higher than 1.
    pub fn support_depth_readback(&self) -> bool {
        match self {
            HardwareTier::Web => false,
            HardwareTier::Native => true,
        }
    }
}

impl Default for HardwareTier {
    fn default() -> Self {
        // Use "Basic" tier for actual web but also if someone forces the GL backend!
        if supported_backends() == wgpu::Backends::GL {
            HardwareTier::Web
        } else {
            HardwareTier::Native
        }
    }
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
            flags: match self {
                HardwareTier::Web => wgpu::DownlevelFlags::empty(),
                // Require fully WebGPU compliance for the native tier.
                HardwareTier::Native => wgpu::DownlevelFlags::all(),
            },
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
    // Only use Vulkan & Metal unless explicitly told so since this reduces surfaces and thus surprises.
    //
    // Bunch of cases where it's still useful to switch though:
    // * Some Windows VMs only provide DX12 drivers, observed with Parallels on Apple Silicon
    // * May run into Linux issues that warrant trying out the GL backend.
    //
    // For changing the backend we use standard wgpu env var, i.e. WGPU_BACKEND.
    #[cfg(not(target_arch = "wasm32"))]
    {
        wgpu::util::backend_bits_from_env()
            .unwrap_or(wgpu::Backends::VULKAN | wgpu::Backends::METAL)
    }
    // Web - we support only WebGL right now, WebGPU should work but hasn't been tested.
    #[cfg(target_arch = "wasm32")]
    {
        wgpu::Backends::GL
    }
}
