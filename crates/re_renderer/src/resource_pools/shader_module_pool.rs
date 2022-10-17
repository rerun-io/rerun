use std::fmt::Display;
use std::sync::atomic::Ordering;
use std::{hash::Hash, path::PathBuf, sync::atomic::AtomicU64};

use ahash::HashSet;

use crate::debug_label::DebugLabel;
use crate::resource_pools::resource_pool::*;
use crate::FileContents;

// ---

slotmap::new_key_type! { pub struct ShaderModuleHandle; }

pub struct ShaderModule {
    last_frame_used: AtomicU64,
    pub last_frame_modified: AtomicU64, // TODO: need associated slotmaps
    pub shader_module: wgpu::ShaderModule,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShaderModuleDesc {
    /// Debug label of the shader.
    /// This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    // TODO(cmc): Cow?
    pub entrypoint: String,
    pub stage: ShaderStage,
    pub source: FileContents,
}
impl Hash for ShaderModuleDesc {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.hash(state)
    }
}
impl ShaderModuleDesc {
    fn label(&self) -> String {
        if let Some(label) = self.label.get() {
            ["shader", &self.stage.to_string(), label].join("/")
        } else {
            ["shader", &self.stage.to_string()].join("/")
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}
impl Display for ShaderStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ShaderStage::Vertex => "vertex",
            ShaderStage::Fragment => "fragment",
            ShaderStage::Compute => "compute",
        })
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
    pub fn request(
        &mut self,
        device: &wgpu::Device,
        desc: &ShaderModuleDesc,
    ) -> ShaderModuleHandle {
        self.pool.get_handle(desc, |desc| {
            // TODO(cmc): https://github.com/gfx-rs/wgpu/issues/2130
            let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: desc.label().as_str().into(),
                // TODO: what about non-WGSL?
                source: wgpu::ShaderSource::Wgsl(
                    desc.source.contents().unwrap().into(), // TODO: handle err
                ),
            });

            ShaderModule {
                last_frame_used: AtomicU64::new(0),
                last_frame_modified: AtomicU64::new(0),
                shader_module,
            }
        })
    }

    pub fn frame_maintenance(
        &mut self,
        device: &wgpu::Device,
        frame_index: u64,
        updated_paths: &HashSet<PathBuf>,
    ) {
        self.pool.discard_unused_resources(frame_index);

        let descs = self.pool.resource_descs().cloned().collect::<Vec<_>>(); // TODO
        for desc in descs {
            let modified = match &desc.source {
                FileContents::Inlined(_) => false,
                FileContents::Path(path) => updated_paths.contains(path),
            };

            if modified {
                println!("rebuilding shader module");

                // TODO: clearly this is horrible ^_^
                let handle = self.pool.get_handle(&desc, |_| unreachable!());
                let res = self.pool.get_resource_mut(handle).unwrap(); // TODO

                // TODO(cmc): https://github.com/gfx-rs/wgpu/issues/2130
                let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: desc.label().as_str().into(),
                    // TODO: what about non-WGSL?
                    source: wgpu::ShaderSource::Wgsl(
                        desc.source.contents().unwrap().into(), // TODO: handle err
                    ),
                });

                res.shader_module = shader_module;
                res.last_frame_modified
                    .store(frame_index + 1, Ordering::Release);
            }
        }
    }

    pub fn get(&self, handle: ShaderModuleHandle) -> Result<&ShaderModule, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: ShaderModuleHandle) {
        let _ = self.get(handle);
    }
}
