use parking_lot::{Mutex, MutexGuard};
use re_renderer::{LineStripSeriesBuilder, PointCloudBuilder};
use re_viewer_context::{ArchetypeDefinition, SceneContextPart};

use crate::scene::{
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES, SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
};

// TODO(wumpf): Workaround for Point & Line builder taking up too much memory to emit them on every scene element that as points/lines.
// If these builders/draw-data would allocate space more dynamically, this would not be necessary!
#[derive(Default)]
pub struct SharedRenderBuilders {
    pub lines: Option<Mutex<LineStripSeriesBuilder>>,
    pub points: Option<Mutex<PointCloudBuilder>>,
}

impl SharedRenderBuilders {
    pub fn lines(&self) -> MutexGuard<'_, LineStripSeriesBuilder> {
        self.lines.as_ref().unwrap().lock()
    }

    pub fn points(&self) -> MutexGuard<'_, PointCloudBuilder> {
        self.points.as_ref().unwrap().lock()
    }
}

impl SceneContextPart for SharedRenderBuilders {
    fn archetypes(&self) -> Vec<ArchetypeDefinition> {
        Vec::new()
    }

    fn populate(
        &mut self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        _query: &re_viewer_context::SceneQuery<'_>,
        _space_view_state: &dyn re_viewer_context::SpaceViewState,
    ) {
        self.lines = Some(Mutex::new(
            LineStripSeriesBuilder::new(&mut ctx.render_ctx.lock())
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES),
        ));
        self.points = Some(Mutex::new(
            PointCloudBuilder::new(&mut ctx.render_ctx.lock())
                .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES),
        ));
    }
}
