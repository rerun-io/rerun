use re_arrow_store::LatestAtQuery;
use re_components::Component;
use re_query::{query_entity_with_primary, QueryError};
use re_viewer_context::{
    ArchetypeDefinition, SceneQuery, SpaceViewClass, SpaceViewHighlights, ViewPartSystem,
    ViewerContext,
};

use crate::TextBoxSpaceView;

// ---

#[derive(Debug, Clone)]
pub struct TextBoxEntry {
    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTextBox {
    pub text_entries: Vec<TextBoxEntry>,
}

impl ViewPartSystem<TextBoxSpaceView> for SceneTextBox {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![re_components::TextBox::name()]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &<TextBoxSpaceView as SpaceViewClass>::State,
        _scene_context: &<TextBoxSpaceView as SpaceViewClass>::Context,
        _highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        let store = &ctx.store_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            match query_entity_with_primary::<re_components::TextBox>(store, &query, ent_path, &[])
                .and_then(|ent_view| {
                    for text_entry in ent_view.iter_primary()?.flatten() {
                        let re_components::TextBox { body } = text_entry;
                        self.text_entries.push(TextBoxEntry { body });
                    }
                    Ok(())
                }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(_) => {
                    re_log::warn_once!("text-box query failed for {ent_path:?}");
                }
            }
        }
        Vec::new()
    }
}
