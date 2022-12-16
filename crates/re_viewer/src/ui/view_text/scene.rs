use re_data_store::{query::visit_type_data_2, FieldName, ObjPath, TimeQuery};
use re_log_types::{IndexHash, MsgId, ObjectType};

use crate::{ui::SceneQuery, ViewerContext};

use super::ui::ViewTextFilters;

// ---

/// A single text entry.
pub struct TextEntry {
    // props
    pub msg_id: MsgId,
    pub obj_path: ObjPath,
    pub time: i64,
    pub color: Option<[u8; 4]>,

    // text entry
    pub level: Option<String>,
    pub body: String,
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
                                time,
                                color: color.copied(),
                                level: level.map(ToOwned::to_owned),
                                body: body.clone(),
                            });
                        }
                    },
                );
            });

        // We want to show the log messages in order.
        // The most important order is the the `time` for whatever timeline we are on.
        // For a tie-breaker, we use MsgId as that is ordered by a high-resolution wall-time.
        crate::profile_scope!("sort");
        self.text_entries
            .sort_by_key(|entry| (entry.time, entry.msg_id));
    }
}
