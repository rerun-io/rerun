use std::any::Any;

use crate::renderer::{DrawData, DrawableCollectionViewInfo};
use crate::{DrawableCollector, RenderContext, RendererTypeId};

/// Utility trait for implementing dynamic dispatch within [`QueueableDrawData`].
pub trait TypeErasedDrawData: Any {
    /// See [`DrawData::collect_drawables`].
    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    );

    /// Returns the name of the renderer that this draw data is associated with.
    fn renderer_name(&self) -> &'static str;

    /// Returns the key of the renderer that this draw data is associated with.
    ///
    /// This also makes sure that the renderer has been initialized already.
    fn renderer_key(&self, ctx: &RenderContext) -> RendererTypeId;
}

impl<D: DrawData + 'static> TypeErasedDrawData for D {
    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        <D as DrawData>::collect_drawables(self, view_info, collector);
    }

    fn renderer_name(&self) -> &'static str {
        std::any::type_name::<D::Renderer>()
    }

    fn renderer_key(&self, ctx: &RenderContext) -> RendererTypeId {
        ctx.renderer::<D::Renderer>().key()
    }
}

/// Type erased draw data that can be submitted directly to the view builder.
pub struct QueueableDrawData(Box<dyn TypeErasedDrawData + Send + Sync>);

impl<D: TypeErasedDrawData + DrawData + Sync + Send + 'static> From<D> for QueueableDrawData {
    fn from(draw_data: D) -> Self {
        Self(Box::new(draw_data))
    }
}

impl std::ops::Deref for QueueableDrawData {
    type Target = dyn TypeErasedDrawData + Send + Sync;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl QueueableDrawData {
    /// Panics if the type `D` is not the underlying type of this draw data.
    #[inline]
    pub(crate) fn expect_downcast<D: DrawData + Any + 'static>(&self) -> &D {
        (self.0.as_ref() as &dyn Any)
            .downcast_ref::<D>()
            .expect("Draw data doesn't have the expected type")
    }
}
