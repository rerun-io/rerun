use std::hash::Hash;
use std::path::PathBuf;

use ahash::HashSet;
use anyhow::Context as _;

use super::static_resource_pool::{StaticResourcePool, StaticResourcePoolReadLockAccessor};
use crate::debug_label::DebugLabel;
use crate::{FileResolver, FileSystem, RenderContext};

// ---

slotmap::new_key_type! { pub struct GpuShaderModuleHandle; }

/// If set, all readily stitched (import resolve) and patched
/// wgsl shaders will be written to the specified directory.
#[cfg(not(target_arch = "wasm32"))]
const RERUN_WGSL_SHADER_DUMP_PATH: &str = "RERUN_WGSL_SHADER_DUMP_PATH";

/// Create a shader module using the `include_file!` macro and set the path name as debug string.
#[macro_export]
macro_rules! include_shader_module {
    ($path:expr $(,)?) => {{
        $crate::ShaderModuleDesc {
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
        resolver: &FileResolver<Fs>,
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

        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(wgsl_dump_dir) = std::env::var(RERUN_WGSL_SHADER_DUMP_PATH) {
            let mut path = PathBuf::from(wgsl_dump_dir);
            std::fs::create_dir_all(&path).unwrap();

            let mut wgsl_filename = self.source.to_str().unwrap().replace(['/', '\\'], "_");
            if let Some(position) = wgsl_filename.find("re_renderer_shader_") {
                wgsl_filename = wgsl_filename[position + "re_renderer_shader_".len()..].to_owned();
            }

            path.push(&wgsl_filename);
            std::fs::write(&path, &source_interpolated.contents).unwrap();
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
    pub fn get_or_create(
        &self,
        ctx: &RenderContext,
        desc: &ShaderModuleDesc,
    ) -> GpuShaderModuleHandle {
        self.pool.get_or_create(desc, |desc| {
            desc.create_shader_module(
                &ctx.device,
                &ctx.resolver,
                &self.shader_text_workaround_replacements,
            )
        })
    }

    pub fn begin_frame<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &FileResolver<Fs>,
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
                paths.extend(source_interpolated.imports);
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

    /// Locks the resource pool for resolving handles.
    ///
    /// While it is locked, no new resources can be added.
    pub fn resources(
        &self,
    ) -> StaticResourcePoolReadLockAccessor<'_, GpuShaderModuleHandle, wgpu::ShaderModule> {
        self.pool.resources()
    }

    pub fn num_resources(&self) -> usize {
        self.pool.num_resources()
    }
}
