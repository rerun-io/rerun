use re_arrow_store::TimeRange;
use re_data_store::{query::visit_type_data_2, FieldName, ObjPath, TimeQuery};
use re_log_types::{
    field_types::{self, Instance},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{range_entity_with_primary, QueryError};

use crate::{ui::SceneQuery, ViewerContext};

use super::ui::ViewTextFilters;

// ---

#[derive(Debug, Clone)]
pub struct TextEntry {
    // props
    pub msg_id: MsgId,
    pub obj_path: ObjPath,
    /// `None` for timeless data.
    pub time: Option<i64>,
    pub color: Option<[u8; 4]>,

    // text entry
    pub level: Option<String>,
    pub body: String,

    // TODO(cmc): remove once legacy store goes away
    pub is_arrow: bool,
}

/// A text scene, with everything needed to render it.
#[derive(Default)]
pub struct SceneText {
    pub text_entries: Vec<TextEntry>,
}

impl SceneText {
    /// Loads all text objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &SceneQuery<'_>,
        filters: &ViewTextFilters,
    ) {
        crate::profile_function!();

        self.load_text_entries(ctx, query, filters);

        self.load_text_entries_arrow(ctx, query, filters);
    }

    fn load_text_entries(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &SceneQuery<'_>,
        filters: &ViewTextFilters,
    ) {
        crate::profile_function!();

        query
            .iter_object_stores(ctx.log_db, &[ObjectType::TextEntry])
            .for_each(|(_obj_type, obj_path, _time_query, obj_store)| {
                // Early filtering: if we're not showing it the view, there isn't much point
                // in querying it to begin with... at least for now.
                if !filters.is_obj_path_visible(obj_path) {
                    return;
                }

                // TODO(cmc): We're cloning full strings here, which is very much a bad idea.
                // We need to change the internal storage so that we store ref-counted strings
                // rather than plain strings.
                //
                // On the other hand:
                // - A) We're about to change our storage engine.
                // - B) Nobody is logging gazillon of text logs into Rerun yet.
                visit_type_data_2(
                    obj_store,
                    &FieldName::from("body"),
                    &TimeQuery::EVERYTHING,
                    ("level", "color"),
                    |_instance_index: Option<&IndexHash>,
                     time: i64,
                     msg_id: &MsgId,
                     body: &String,
                     level: Option<&String>,
                     color: Option<&[u8; 4]>| {
                        // Early filtering once more, see above.
                        let is_visible = level
                            .as_ref()
                            .map_or(true, |lvl| filters.is_log_level_visible(lvl));

                        if is_visible {
                            self.text_entries.push(TextEntry {
                                msg_id: *msg_id,
                                obj_path: obj_path.clone(),
                                time: time.into(),
                                color: color.copied(),
                                level: level.map(ToOwned::to_owned),
                                body: body.clone(),
                                is_arrow: false,
                            });
                        }
                    },
                );
            });

        // We want to show the log messages in order.
        // The most important order is the the `time` for whatever timeline we are on.
        // For a tie-breaker, we use MsgId as that is ordered by a high-resolution wall-time.
        crate::profile_scope!("sort");
        self.text_entries.sort_by_key(|te| (te.time, te.msg_id));
    }

    fn load_text_entries_arrow(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &SceneQuery<'_>,
        filters: &ViewTextFilters,
    ) {
        crate::profile_function!();

        let store = &ctx.log_db.obj_db.arrow_store;

        for obj_path in query.obj_paths {
            let ent_path = obj_path;

            // Early filtering: if we're not showing it the view, there isn't much point
            // in querying it to begin with... at least for now.
            if !filters.is_obj_path_visible(ent_path) {
                return;
            }

            let query = re_arrow_store::RangeQuery::new(
                query.timeline,
                TimeRange::new(i64::MIN.into(), i64::MAX.into()),
            );

            let components = [
                Instance::name(),
                MsgId::name(),
                field_types::TextEntry::name(),
                field_types::ColorRGBA::name(),
            ];
            let ent_views = range_entity_with_primary::<field_types::TextEntry, 4>(
                store, query, ent_path, components,
            );

            for (time, ent_view) in ent_views {
                match ent_view.visit3(
                    |_instance,
                     text_entry: field_types::TextEntry,
                     msg_id: Option<MsgId>,
                     color: Option<field_types::ColorRGBA>| {
                        let field_types::TextEntry { body, level } = text_entry;

                        // Early filtering once more, see above.
                        let is_visible = level
                            .as_ref()
                            .map_or(true, |lvl| filters.is_log_level_visible(lvl));

                        if is_visible {
                            self.text_entries.push(TextEntry {
                                msg_id: msg_id.unwrap(), // always present
                                obj_path: obj_path.clone(),
                                time: time.map(|time| time.as_i64()),
                                color: color.map(|c| c.to_array()),
                                level,
                                body,
                                is_arrow: true,
                            });
                        }
                    },
                ) {
                    Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                    Err(err) => {
                        re_log::error_once!("Unexpected error querying '{ent_path:?}': {err:?}");
                    }
                }
            }
        }
    }
}
