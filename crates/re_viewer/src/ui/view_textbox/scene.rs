use eframe::epaint::text;
use re_arrow_store::{LatestAtQuery, TimeRange};
use re_data_store::EntityPath;
use re_log::warn_once;
use re_log_types::{
    component_types::{self, InstanceKey},
    Component, RowId,
};
use re_query::{query_entity_with_primary, range_entity_with_primary, QueryError};

use crate::{ui::SceneQuery, ViewerContext};

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
            let ent_view = query_entity_with_primary::<component_types::TextEntry>(
                store,
                &query,
                ent_path,
                &[],
            );
            let Ok(ent_view) = ent_view else {
                warn_once!("textbox query failed for {ent_path:?}");
                continue;
            };
            let Ok(text_entries) = ent_view.iter_primary() else {
                warn_once!("textbox query failed for {ent_path:?}");
                continue;
            };

            for text_entry in text_entries.flatten() {
                let component_types::TextEntry { body, level: _ } = text_entry;
                self.text_entries.push(TextboxEntry { body });
            }
        }
    }
}
