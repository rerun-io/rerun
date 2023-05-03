use std::{hash::Hash, path::PathBuf};

use ahash::HashSet;
use anyhow::Context as _;

use crate::{debug_label::DebugLabel, FileResolver, FileSystem};

use super::{
    resource::{PoolError, ResourceStatistics},
    static_resource_pool::StaticResourcePool,
};

// ---

slotmap::new_key_type! { pub struct GpuShaderModuleHandle; }

/// Create a shader module using the `include_file!` macro and set the path name as debug string.
#[macro_export]
macro_rules! include_shader_module {
    ($path:expr $(,)?) => {{
        $crate::wgpu_resources::ShaderModuleDesc {
            label: $crate::DebugLabel::from(stringify!($path).strip_prefix("../../shader/")),
            source: $crate::include_file!($path),
            extra_workaround_replacements: Vec::new(),
        }
    }};
}

#[derive(Clone, Eq, Debug)]
pub struct ShaderModuleDesc {
    /// Debug label of the shader.
    /// This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Path to the source code of this shader module.
    pub source: PathBuf,

    /// Additional text replacement workarounds that may be added on top of globally known workarounds.
    pub extra_workaround_replacements: Vec<(String, String)>,
}

impl PartialEq for ShaderModuleDesc {
    fn eq(&self, rhs: &Self) -> bool {
        self.source.eq(&rhs.source)
    }
}

impl Hash for ShaderModuleDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // NOTE: for a shader, the only thing that should matter is the source
        // code since we can have many entrypoints referring to the same file!
        self.source.hash(state);
        self.extra_workaround_replacements.hash(state);
    }
}

impl ShaderModuleDesc {
    fn create_shader_module<Fs: FileSystem>(
        &self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        shader_text_workaround_replacements: &[(String, String)],
    ) -> wgpu::ShaderModule {
        let mut source_interpolated = resolver
            .populate(&self.source)
            .context("couldn't resolve shader module's contents")
            .map_err(|err| re_log::error!(err=%re_error::format(err)))
            .unwrap_or_default();

        for (from, to) in shader_text_workaround_replacements
            .iter()
            .chain(self.extra_workaround_replacements.iter())
        {
            source_interpolated.contents = source_interpolated.contents.replace(from, to);
        }

        // All wgpu errors come asynchronously: this call will succeed whether the given
        // source is valid or not.
        // Only when actually submitting passes that make use of this shader will we know if
        // something is wrong or not.
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: self.label.get(),
            // TODO(cmc): handle non-WGSL shaders.
            source: wgpu::ShaderSource::Wgsl(source_interpolated.contents.into()),
        })
    }
}

// ---

#[derive(Default)]
pub struct GpuShaderModulePool {
    pool: StaticResourcePool<GpuShaderModuleHandle, ShaderModuleDesc, wgpu::ShaderModule>,

    /// Workarounds via text replacement in shader source code.
    ///
    /// TODO(andreas): These should be solved with a pre-processor.
    pub shader_text_workaround_replacements: Vec<(String, String)>,
}

impl GpuShaderModulePool {
    pub fn get_or_create<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        desc: &ShaderModuleDesc,
    ) -> GpuShaderModuleHandle {
        self.pool.get_or_create(desc, |desc| {
            desc.create_shader_module(device, resolver, &self.shader_text_workaround_replacements)
        })
    }

    pub fn begin_frame<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        frame_index: u64,
        updated_paths: &HashSet<PathBuf>,
    ) {
        self.pool.current_frame_index = frame_index;

        if updated_paths.is_empty() {
            return;
        }

        // Recompile all shader that refer to paths modified since last frame.
        self.pool.recreate_resources(|desc| {
            // Not only do we care about filesystem events that touch upon the source
            // path of the current shader, we also care about events that affect any of
            // our direct and indirect dependencies (#import)!
            let mut paths = vec![desc.source.clone()];
            if let Ok(source_interpolated) = resolver.populate(&desc.source) {
                paths.extend(source_interpolated.imports.into_iter());
            }

            paths.iter().any(|p| updated_paths.contains(p)).then(|| {
                let shader_module = desc.create_shader_module(
                    device,
                    resolver,
                    &self.shader_text_workaround_replacements,
                );
                re_log::debug!(?desc.source, label = desc.label.get(), "recompiled shader module");
                shader_module
            })
        });
    }

    pub fn get(&self, handle: GpuShaderModuleHandle) -> Result<&wgpu::ShaderModule, PoolError> {
        self.pool.get_resource(handle)
    }

    pub fn get_statistics(
        &self,
        handle: GpuShaderModuleHandle,
    ) -> Result<&ResourceStatistics, PoolError> {
        self.pool.get_statistics(handle)
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
