use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use re_renderer::{LineStripSeriesBuilder, PointCloudBuilder, RenderContext};
use re_types::ComponentNameSet;
use re_viewer_context::{IdentifiedViewSystem, ViewContextSystem};

use crate::parts::{
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
};

// TODO(wumpf): Workaround for Point & Line builder taking up too much memory to emit them on every scene element that as points/lines.
// If these builders/draw-data would allocate space more dynamically, this would not be necessary!
#[derive(Default)]
pub struct SharedRenderBuilders {
    pub lines: Mutex<Option<LineStripSeriesBuilder>>,
    pub points: Mutex<Option<PointCloudBuilder>>,
}

impl IdentifiedViewSystem for SharedRenderBuilders {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "SharedRenderBuilders".into()
    }
}

impl SharedRenderBuilders {
    pub fn lines(&self) -> MappedMutexGuard<'_, LineStripSeriesBuilder> {
        MutexGuard::map(self.lines.lock(), |l| l.as_mut().unwrap())
    }

    pub fn points(&self) -> MappedMutexGuard<'_, PointCloudBuilder> {
        MutexGuard::map(self.points.lock(), |l| l.as_mut().unwrap())
    }

    pub fn queuable_draw_data(
        &self,
        render_ctx: &mut RenderContext,
    ) -> Vec<re_renderer::QueueableDrawData> {
        let mut result = Vec::new();
        result.extend(
            self.lines
                .lock()
                .take()
                .and_then(|l| match l.into_draw_data(render_ctx) {
                    Ok(d) => Some(d.into()),
                    Err(err) => {
                        re_log::error_once!("Failed to build line strip draw data: {err}");
                        None
                    }
                }),
        );
        result.extend(
            self.points
                .lock()
                .take()
                .and_then(|l| match l.into_draw_data(render_ctx) {
                    Ok(d) => Some(d.into()),
                    Err(err) => {
                        re_log::error_once!("Failed to build point draw data: {err}");
                        None
                    }
                }),
        );
        result
    }
}

impl ViewContextSystem for SharedRenderBuilders {
    fn compatible_component_sets(&self) -> Vec<ComponentNameSet> {
        Vec::new()
    }

    fn execute(
        &mut self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        _query: &re_viewer_context::ViewQuery<'_>,
    ) {
        re_tracing::profile_function!();
        self.lines = Mutex::new(Some(
            LineStripSeriesBuilder::new(ctx.render_ctx)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES),
        ));
        self.points = Mutex::new(Some(
            PointCloudBuilder::new(ctx.render_ctx)
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES),
        ));
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
