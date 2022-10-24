use std::sync::atomic::Ordering;
use std::{hash::Hash, path::PathBuf, sync::atomic::AtomicU64};

use ahash::HashSet;
use anyhow::Context as _;

use crate::debug_label::DebugLabel;
use crate::resource_pools::resource_pool::*;
use crate::{FileContentsHandle, FileResolver, FileSystem};

// ---

slotmap::new_key_type! { pub struct ShaderModuleHandle; }

pub struct ShaderModule {
    last_frame_used: AtomicU64,
    pub last_frame_modified: AtomicU64, // TODO(cmc): need associated slotmaps
    pub shader_module: wgpu::ShaderModule,
}

#[derive(Clone, Eq, Debug)]
pub struct ShaderModuleDesc {
    /// Debug label of the shader.
    /// This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    /// Actual source code of the shader module.
    pub source: FileContentsHandle,
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
    ) -> anyhow::Result<wgpu::ShaderModule> {
        let code = self
            .source
            .resolve_contents(resolver)
            .context("couldn't resolve file contents")?;

        // All wgpu errors come asynchronously: this call will succeed whether the given
        // source is valid or not.
        // Only when building an actual pipeline using this shader will we know if
        // something is wrong.
        Ok(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: self.label.get(),
            // TODO(cmc): handle non-WGSL shaders.
            source: wgpu::ShaderSource::Wgsl(code),
        }))
    }
}

impl UsageTrackedResource for ShaderModule {
    fn last_frame_used(&self) -> &AtomicU64 {
        &self.last_frame_used
    }
}

// ---

#[derive(Default)]
pub struct ShaderModulePool {
    pool: ResourcePool<ShaderModuleHandle, ShaderModuleDesc, ShaderModule>,
}

impl ShaderModulePool {
    pub fn request<Fs: FileSystem>(
        &mut self,
        device: &wgpu::Device,
        resolver: &mut FileResolver<Fs>,
        desc: &ShaderModuleDesc,
    ) -> ShaderModuleHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(cmc): must provide a way to properly handle errors in requests.
            // Probably better to wait for a first PoC of #import to land though,
            // as that will surface a bunch of shortcomings in our error handling too.
            let shader_module = desc.create_shader_module(device, resolver).unwrap();
            ShaderModule {
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
        // Garbage collect shaders not being used by any pipeline.
        self.pool.discard_unused_resources(frame_index);

        // All shader descriptors that refer to paths modified since last frame.
        let descs = self
            .pool
            .resource_descs()
            .filter_map(|desc| {
                (!updated_paths.is_empty()).then(|| match &desc.source {
                    FileContentsHandle::Inlined(_) => None,
                    FileContentsHandle::Path(path) => {
                        let mut paths = vec![path.as_path()];
                        if let Ok(imports) = resolver.resolve_imports(path) {
                            paths.extend(imports);
                        }

                        paths
                            .iter()
                            .any(|p| updated_paths.contains(*p))
                            .then_some((path, desc))
                    }
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
            let handle = self.pool.get_handle(&desc, |_| {
                unreachable!("the pool itself handed us that descriptor")
            });
            let res = self
                .pool
                .get_resource_mut(handle)
                .expect("the pool itself handed us that handle");

            let shader_module = match desc.create_shader_module(device, resolver) {
                Ok(sm) => sm,
                Err(err) => {
                    re_log::error!(
                        err = re_error::format(err),
                        ?path,
                        "couldn't recompile shader module"
                    );
                    continue;
                }
            };

            re_log::debug!(
                ?path,
                label = desc.label.get(),
                "successfully recompiled shader module"
            );

            res.shader_module = shader_module;
            res.last_frame_modified
                // NOTE: we add an extra frame here because render pipeline maintenance
                // has already run for the current frame.
                .store(frame_index + 1, Ordering::Release);
        }
    }

    pub fn get(&self, handle: ShaderModuleHandle) -> Result<&ShaderModule, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: ShaderModuleHandle) {
        let _ = self.get(handle);
    }
}
