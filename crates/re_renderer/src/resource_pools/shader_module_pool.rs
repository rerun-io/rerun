use std::sync::atomic::Ordering;
use std::{hash::Hash, path::PathBuf, sync::atomic::AtomicU64};

use ahash::HashSet;
use anyhow::Context as _;

use crate::{debug_label::DebugLabel, FileResolver, FileSystem};

use super::{resource::*, static_resource_pool::StaticResourcePool};

// ---

slotmap::new_key_type! { pub struct GpuShaderModuleHandle; }

pub struct GpuShaderModule {
    last_frame_used: AtomicU64,
    pub last_frame_modified: AtomicU64, // TODO(cmc): need associated slotmaps
    pub shader_module: wgpu::ShaderModule,
}

#[derive(Clone, Eq, Debug)]
pub struct ShaderModuleDesc {
    /// Debug label of the shader.
    /// This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Path to the source code of this shader module.
    pub source: PathBuf,
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
    }
}
impl ShaderModuleDesc {
    fn create_shader_module<Fs: FileSystem>(
        &self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
    ) -> wgpu::ShaderModule {
        let code = resolver
            .resolve_contents(&self.source)
            .map(|s| s.to_owned())
            .context("couldn't resolve shader module's contents")
            .map_err(|err| re_log::error!(err=%re_error::format(err)))
            .unwrap_or_default();

        // All wgpu errors come asynchronously: this call will succeed whether the given
        // source is valid or not.
        // Only when actually submitting passes that make use of this shader will we know if
        // something is wrong or not.
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: self.label.get(),
            // TODO(cmc): handle non-WGSL shaders.
            source: wgpu::ShaderSource::Wgsl(code.into()),
        })
    }
}

impl UsageTrackedResource for GpuShaderModule {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

// ---

#[derive(Default)]
pub struct GpuShaderModulePool {
    pool: StaticResourcePool<GpuShaderModuleHandle, ShaderModuleDesc, GpuShaderModule>,
}

impl GpuShaderModulePool {
    pub fn get_or_create<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        desc: &ShaderModuleDesc,
    ) -> GpuShaderModuleHandle {
        self.pool.get_or_create(desc, |desc| {
            let shader_module = desc.create_shader_module(device, resolver);
            GpuShaderModule {
                last_frame_used: AtomicU64::new(0),
                last_frame_modified: AtomicU64::new(0),
                shader_module,
            }
        })
    }

    pub fn frame_maintenance<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        frame_index: u64,
        updated_paths: &HashSet<PathBuf>,
    ) {
        // All shader descriptors that refer to paths modified since last frame.
        let descs = self
            .pool
            .resource_descs()
            .filter_map(|desc| {
                // Not only do we care about filesystem events that touch upon the source
                // path of the current shader, we also care about events that affect any of
                // our direct and indirect dependencies (#import)!
                (!updated_paths.is_empty()).then(|| {
                    let mut paths = vec![desc.source.as_path()];
                    if let Ok(imports) = resolver.resolve_imports(&desc.source) {
                        paths.extend(imports);
                    }

                    paths
                        .iter()
                        .any(|p| updated_paths.contains(*p))
                        .then_some((&desc.source, desc))
                })
            })
            .flatten()
            // TODO(cmc): clearly none of this is nice, but I don't want try and figure
            // out better APIs until #import has landed, as that will probably point out
            // a whole bunch of shortcomings in our APIs too.
            .map(|(path, desc)| (path.clone(), desc.clone()))
            .collect::<Vec<_>>();

        // Recompile shader modules for outdated descriptors.
        for (path, desc) in descs {
            // TODO(cmc): obviously terrible, we'll see as things evolve.
            let handle = self.pool.get_or_create(&desc, |_| {
                unreachable!("the pool itself handed us that descriptor")
            });
            let res = self
                .pool
                .get_resource_mut(handle)
                .expect("the pool itself handed us that handle");

            re_log::debug!(?path, label = desc.label.get(), "recompiled shader module");

            res.shader_module = desc.create_shader_module(device, resolver);
            res.last_frame_modified
                // NOTE: we add an extra frame here because render pipeline maintenance
                // has already run for the current frame.
                .store(frame_index + 1, Ordering::Release);
        }
    }

    pub fn get(&self, handle: GpuShaderModuleHandle) -> Result<&GpuShaderModule, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: GpuShaderModuleHandle) {
        let _ = self.get(handle);
    }
}
