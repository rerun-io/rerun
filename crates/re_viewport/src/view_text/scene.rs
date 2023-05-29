use re_arrow_store::TimeRange;
use re_data_store::EntityPath;
use re_log_types::{
    component_types::{self, InstanceKey},
    Component, RowId,
};
use re_query::{range_entity_with_primary, QueryError};
use re_viewer_context::{SceneQuery, ViewerContext};

use super::ui::ViewTextFilters;

// ---

#[derive(Debug, Clone)]
pub struct TextEntry {
    // props
    pub row_id: RowId,

    pub entity_path: EntityPath,

    /// `None` for timeless data.
    pub time: Option<i64>,

    pub color: Option<[u8; 4]>,

    pub level: Option<String>,

    pub body: String,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneText {
    pub text_entries: Vec<TextEntry>,
}

impl SceneText {
    /// Loads all text components into the scene according to the given query.
    pub(crate) fn load(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &SceneQuery<'_>,
        filters: &ViewTextFilters,
    ) {
        crate::profile_function!();

        let store = &ctx.log_db.entity_db.data_store;

        for entity_path in query.entity_paths {
            let ent_path = entity_path;

            // Early filtering: if we're not showing it the view, there isn't much point
            // in querying it to begin with... at least for now.
            if !filters.is_entity_path_visible(ent_path) {
                return;
            }

            let query = re_arrow_store::RangeQuery::new(
                query.timeline,
                TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            );

            let components = [
                InstanceKey::name(),
                component_types::TextEntry::name(),
                component_types::ColorRGBA::name(),
            ];
            let ent_views = range_entity_with_primary::<component_types::TextEntry, 3>(
                store, &query, ent_path, components,
            );

            for (time, ent_view) in ent_views {
                match ent_view.visit2(
                    |_instance,
                     text_entry: component_types::TextEntry,
                     color: Option<component_types::ColorRGBA>| {
                        let component_types::TextEntry { body, level } = text_entry;

                        // Early filtering once more, see above.
                        let is_visible = level
                            .as_ref()
                            .map_or(true, |lvl| filters.is_log_level_visible(lvl));

                        if is_visible {
                            self.text_entries.push(TextEntry {
                                row_id: ent_view.row_id(),
                                entity_path: entity_path.clone(),
                                time: time.map(|time| time.as_i64()),
                                color: color.map(|c| c.to_array()),
                                level,
                                body,
                            });
                        }
                    },
                ) {
                    Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                    Err(err) => {
                        re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                    }
                }
            }
        }
    }
}
