use std::fmt::Display;
use std::{hash::Hash, path::PathBuf, sync::atomic::AtomicU64};

use crate::debug_label::DebugLabel;
use crate::resource_pools::resource_pool::*;

// ---

slotmap::new_key_type! { pub struct ShaderModuleHandle; }

pub struct ShaderModule {
    last_frame_used: AtomicU64,
    pub shader_module: wgpu::ShaderModule,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct ShaderModuleDesc {
    /// Debug label of the shader.
    /// This will show up in graphics debuggers for easy identification.
    pub label: DebugLabel,

    // TODO(cmc): Cow?
    pub entrypoint: String,
    pub stage: ShaderStage,
    pub source: ShaderSource,
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

// TODO(cmc): This abstraction probably does not belong here.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum ShaderSource {
    /// Inlined shader source code.
    // TODO(cmc): what about non-WGSL?
    Wgsl { data: String },
    /// Filesystem path.
    Path { path: PathBuf },
    // TODO(cmc): fetch over network?
}
impl ShaderSource {
    pub fn from_wgsl(wgsl: &str) -> Self {
        Self::Wgsl { data: wgsl.into() }
    }

    pub fn data(&self) -> &str {
        match self {
            ShaderSource::Wgsl { data } => data,
            ShaderSource::Path { path: _ } => todo!(),
        }
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
            let label = if let Some(label) = desc.label.get() {
                ["shader", &desc.stage.to_string(), label].join("/")
            } else {
                ["shader", &desc.stage.to_string()].join("/")
            };
            // TODO(cmc): https://github.com/gfx-rs/wgpu/issues/2130
            let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: label.as_str().into(),
                source: wgpu::ShaderSource::Wgsl(desc.source.data().into()),
            });
            ShaderModule {
                last_frame_used: AtomicU64::new(0),
                shader_module,
            }
        })
    }

    pub fn frame_maintenance(&mut self, frame_index: u64) {
        self.pool.discard_unused_resources(frame_index);
    }

    pub fn get(&self, handle: ShaderModuleHandle) -> Result<&ShaderModule, PoolError> {
        self.pool.get_resource(handle)
    }

    pub(super) fn register_resource_usage(&mut self, handle: ShaderModuleHandle) {
        let _ = self.get(handle);
    }
}
