use re_renderer::RenderContext;
use re_renderer::external::wgpu;
use re_renderer::resource_managers::GpuTexture3D;
use wgpu::util::DeviceExt as _;

/// A custom bind group for user-provided shader parameters.
///
/// Holds the uniform buffer and texture bindings that the custom fragment shader needs.
pub struct CustomShaderBindGroup {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

/// Build a custom bind group from resolved shader parameters.
///
/// Layout:
/// - Binding 0: Uniform buffer with packed parameter data
/// - Texture bindings at indices specified in the `textures_3d` parameter
///   (must not conflict with binding 0 when uniforms are present)
/// - Sampler bindings at texture binding + 100
///
/// Returns `None` if binding validation fails.
pub fn build_custom_bind_group(
    ctx: &RenderContext,
    label: &str,
    uniform_data: &[u8],
    textures_3d: &[(u32, GpuTexture3D)],
) -> Option<CustomShaderBindGroup> {
    let device = &ctx.device;

    // Validate binding indices: no duplicates, no conflicts with binding 0 when uniforms exist
    {
        let has_uniforms = !uniform_data.is_empty();
        let mut used_bindings: Vec<u32> = Vec::new();
        if has_uniforms {
            used_bindings.push(0);
        }
        for (binding, _) in textures_3d {
            if used_bindings.contains(binding) {
                re_log::warn_once!(
                    "Custom shader bind group '{label}': duplicate binding index {binding}"
                );
                return None;
            }
            used_bindings.push(*binding);

            let sampler_binding = binding + 100;
            if used_bindings.contains(&sampler_binding) {
                re_log::warn_once!(
                    "Custom shader bind group '{label}': sampler binding {sampler_binding} conflicts"
                );
                return None;
            }
            used_bindings.push(sampler_binding);
        }
    }

    // Build layout entries
    let mut layout_entries = Vec::new();

    // Uniform buffer at binding 0 (if there are uniforms)
    let uniform_buffer = if !uniform_data.is_empty() {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} uniform buffer")),
            contents: uniform_data,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });

        Some(buffer)
    } else {
        None
    };

    // 3D texture bindings
    for (binding, _texture) in textures_3d {
        // R32Float is not filterable on most GPUs, so use non-filterable sample type
        // and a non-filtering sampler.
        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: *binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension: wgpu::TextureViewDimension::D3,
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
            },
            count: None,
        });

        // Also add a sampler for this texture at binding + 100
        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: *binding + 100,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count: None,
        });
    }

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(&format!("{label} bind group layout")),
        entries: &layout_entries,
    });

    // Build bind group entries
    let mut bind_group_entries = Vec::new();

    if let Some(buffer) = &uniform_buffer {
        bind_group_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        });
    }

    // Create texture views and samplers for 3D textures
    let mut texture_views = Vec::new();
    let mut samplers = Vec::new();

    for (_, texture) in textures_3d {
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        });
        texture_views.push(view);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{label} sampler")),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        samplers.push(sampler);
    }

    for (i, (binding, _)) in textures_3d.iter().enumerate() {
        bind_group_entries.push(wgpu::BindGroupEntry {
            binding: *binding,
            resource: wgpu::BindingResource::TextureView(&texture_views[i]),
        });
        bind_group_entries.push(wgpu::BindGroupEntry {
            binding: *binding + 100,
            resource: wgpu::BindingResource::Sampler(&samplers[i]),
        });
    }

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label} bind group")),
        layout: &bind_group_layout,
        entries: &bind_group_entries,
    });

    Some(CustomShaderBindGroup {
        bind_group_layout,
        bind_group,
    })
}
