use re_arrow_store::LatestAtQuery;
use re_log::warn_once;
use re_log_types::component_types::{self};
use re_query::{query_entity_with_primary, QueryError};
use re_viewer_context::{SceneQuery, ViewerContext};

// ---

#[derive(Debug, Clone)]
pub struct TextboxEntry {
    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneTextbox {
    pub text_entries: Vec<TextboxEntry>,
}

impl SceneTextbox {
    /// Loads all text components into the scene according to the given query.
    pub(crate) fn load(&mut self, ctx: &ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            match query_entity_with_primary::<component_types::Textbox>(
                store,
                &query,
                ent_path,
                &[],
            )
            .and_then(|ent_view| {
                for text_entry in ent_view.iter_primary()?.flatten() {
                    let component_types::Textbox { body } = text_entry;
                    self.text_entries.push(TextboxEntry { body });
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(_) => {
                    warn_once!("textbox query failed for {ent_path:?}");
                }
            }
        }
    }
}
