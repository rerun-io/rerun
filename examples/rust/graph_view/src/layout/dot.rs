use std::collections::HashMap;

use layout::{
    core::{
        base::Orientation,
        format::{ClipHandle, RenderBackend},
        geometry::Point,
        style::StyleAttr,
    },
    std_shapes::shapes::{Arrow, Element, ShapeKind},
    topo::layout::VisualGraph,
};
use re_viewer::external::egui;

use crate::{error::Error, types::NodeIndex};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DotLayout;

impl DotLayout {
    pub fn compute(
        &self,
        nodes: impl IntoIterator<Item = (NodeIndex, egui::Vec2)>,
        directed: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
        undirected: impl IntoIterator<Item = (NodeIndex, NodeIndex)>,
    ) -> Result<HashMap<NodeIndex, egui::Rect>, Error> {
        let mut handle_to_ix = HashMap::new();
        let mut ix_to_handle = HashMap::new();

        let mut graph = VisualGraph::new(Orientation::TopToBottom);

        for (ix, size) in nodes {
            let size = Point::new(size.x as f64, size.y as f64);
            let handle = graph.add_node(Element::create(
                ShapeKind::new_box("test"),
                StyleAttr::simple(),
                Orientation::LeftToRight,
                size,
            ));
            handle_to_ix.insert(handle, ix.clone());
            ix_to_handle.insert(ix, handle);
        }

        for (source_ix, target_ix) in directed {
            let source = ix_to_handle
                .get(&source_ix)
                .ok_or_else(|| Error::EdgeUnknownNode(source_ix.to_string()))?;
            let target = ix_to_handle
                .get(&target_ix)
                .ok_or_else(|| Error::EdgeUnknownNode(target_ix.to_string()))?;
            graph.add_edge(Arrow::simple("test"), *source, *target);
        }

        for (source_ix, target_ix) in undirected {
            let source = ix_to_handle
                .get(&source_ix)
                .ok_or_else(|| Error::EdgeUnknownNode(source_ix.to_string()))?;
            let target = ix_to_handle
                .get(&target_ix)
                .ok_or_else(|| Error::EdgeUnknownNode(target_ix.to_string()))?;

            // TODO(grtlr): find a better way other than adding duplicate edges.
            graph.add_edge(Arrow::simple("test"), *source, *target);
            graph.add_edge(Arrow::simple("test"), *target, *source);
        }

        graph.do_it(false, false, false, &mut DummyBackend);

        let res = handle_to_ix
            .into_iter()
            .map(|(h, ix)| {
                let (min, max) = graph.pos(h).bbox(false);
                (
                    ix,
                    egui::Rect::from_min_max(
                        egui::Pos2::new(min.x as f32, min.y as f32),
                        egui::Pos2::new(max.x as f32, max.y as f32),
                    ),
                )
            })
            .collect();

        Ok(res)
    }
}

struct DummyBackend;

impl RenderBackend for DummyBackend {
    fn draw_rect(
        &mut self,
        _xy: Point,
        _size: Point,
        _look: &StyleAttr,
        _clip: Option<layout::core::format::ClipHandle>,
    ) {
    }

    fn draw_line(&mut self, _start: Point, _stop: Point, _look: &StyleAttr) {}

    fn draw_circle(&mut self, _xy: Point, _size: Point, _look: &StyleAttr) {}

    fn draw_text(&mut self, _xy: Point, _text: &str, _look: &StyleAttr) {}

    fn draw_arrow(
        &mut self,
        _path: &[(Point, Point)],
        _dashed: bool,
        _head: (bool, bool),
        _look: &StyleAttr,
        _text: &str,
    ) {
    }

    fn create_clip(&mut self, _xy: Point, _size: Point, _rounded_px: usize) -> ClipHandle {
        ClipHandle::default()
    }
}
