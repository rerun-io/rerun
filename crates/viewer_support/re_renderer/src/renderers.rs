use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::RenderContext;
use crate::renderer::{Renderer, RendererExt};

#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum RendererRegistrationError {
    #[error("Renderer type {renderer_name} was not registered")]
    NotRegistered { renderer_name: &'static str },

    #[error("Renderer type {renderer_name} did not match its registry entry")]
    TypeMismatch { renderer_name: &'static str },
}

impl RendererRegistrationError {
    fn not_registered<R: Renderer + 'static>() -> Self {
        Self::NotRegistered {
            renderer_name: std::any::type_name::<R>(),
        }
    }

    fn type_mismatch<R: Renderer + 'static>() -> Self {
        Self::TypeMismatch {
            renderer_name: std::any::type_name::<R>(),
        }
    }
}

/// Unique identifier for a [`Renderer`] type.
///
/// We generally don't expect many different distinct types of renderers,
/// therefore 255 should be more than enough.
/// This limitation simplifies sorting of drawables a bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RendererTypeId(u8);

impl RendererTypeId {
    #[inline]
    pub const fn bits(&self) -> u8 {
        self.0
    }

    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

struct RendererEntry {
    renderer: OnceLock<Box<dyn RendererExt>>,
    key: RendererTypeId,
    name: &'static str,
}

/// Registry of all available [`Renderer`] types.
///
/// Renderer types are registered before the context is shared, then initialized independently on first access.
/// This keeps steady-state renderer access free of synchronization between renderer types.
#[derive(Default)]
pub struct Renderers {
    renderer_entries: HashMap<TypeId, Arc<RendererEntry>>,
    renderer_entries_by_key: Vec<Arc<RendererEntry>>,
}

impl Renderers {
    /// Registers a renderer without initializing it.
    ///
    /// Initialization happens on first use.
    /// Registering more than 256 distinct renderer types logs an error, but otherwise fails silently.
    pub fn register<R: Renderer + Send + Sync + 'static>(&mut self) {
        let type_id = TypeId::of::<R>();
        if self.renderer_entries.contains_key(&type_id) {
            return;
        }

        let Ok(key) = self.renderer_entries_by_key.len().try_into() else {
            re_log::error!("Supporting at most 256 distinct renderer types.");
            return;
        };
        let key = RendererTypeId(key);
        let entry = Arc::new(RendererEntry {
            renderer: OnceLock::new(),
            key,
            name: std::any::type_name::<R>(),
        });

        let previous = self.renderer_entries.insert(type_id, entry.clone());
        re_log::debug_assert!(previous.is_none());
        self.renderer_entries_by_key.push(entry);
    }

    /// Gets a registered renderer, initializing it if necessary.
    pub fn get<R: Renderer + Send + Sync + 'static>(
        &self,
        ctx: &RenderContext,
    ) -> Result<&R, RendererRegistrationError> {
        let entry = self
            .renderer_entries
            .get(&TypeId::of::<R>())
            .ok_or_else(RendererRegistrationError::not_registered::<R>)?;
        let renderer = entry.renderer.get_or_init(|| {
            re_tracing::profile_scope!("create_renderer", std::any::type_name::<R>());
            Box::new(R::create_renderer(ctx))
        });
        (renderer.as_ref() as &dyn Any)
            .downcast_ref::<R>()
            .ok_or_else(RendererRegistrationError::type_mismatch::<R>)
    }

    /// Gets the key assigned to a registered renderer.
    ///
    /// Does not initialize the renderer.
    pub fn get_key<R: Renderer + Send + Sync + 'static>(
        &self,
    ) -> Result<RendererTypeId, RendererRegistrationError> {
        self.renderer_entries
            .get(&TypeId::of::<R>())
            .map(|entry| entry.key)
            .ok_or_else(RendererRegistrationError::not_registered::<R>)
    }

    /// Gets an initialized, type-erased renderer by its key.
    pub fn get_by_key(&self, key: RendererTypeId) -> Option<(&'static str, &dyn RendererExt)> {
        let entry = self.renderer_entries_by_key.get(key.0 as usize)?;
        let renderer = entry.renderer.get()?;
        Some((entry.name, renderer.as_ref()))
    }

    /// Returns a remap table where `remap[key.bits()]` is the `u8` sort key for that renderer.
    ///
    /// The sort key is the rank of each renderer's Rust type name among all currently registered
    /// renderers, compared lexicographically.
    /// Because type names are stable within a build, the relative ordering of any two renderers is
    /// too — regardless of which was registered first in this session.
    /// This is what makes draw order deterministic across sessions.
    pub(crate) fn name_sort_remap(&self) -> [u8; 256] {
        let mut remap = [0u8; 256];
        let mut pairs: smallvec::SmallVec<[(&'static str, u8); 16]> = self
            .renderer_entries_by_key
            .iter()
            .enumerate()
            .map(|(key, entry)| (entry.name, key as u8))
            .collect();
        pairs.sort_by_key(|&(name, _)| name);

        for (rank, (_name, key)) in pairs.into_iter().enumerate() {
            remap[key as usize] = rank as u8;
        }

        remap
    }
}
