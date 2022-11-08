use crate::{ui::SceneQuery, ViewerContext};
use re_data_store::{
    query::visit_type_data_3, FieldName, ObjPath, ObjectTreeProperties, TimeQuery,
};
use re_log_types::{IndexHash, MsgId, ObjectType};

// ---

/// A single text entry as part of a whole text scene.
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
    pub(crate) fn load(
        &mut self,
        ctx: &ViewerContext<'_>,
        obj_tree_props: &ObjectTreeProperties,
        query: &SceneQuery<'_>,
    ) {
        crate::profile_function!();

        {
            crate::profile_scope!("SceneText - load text entries");
            let text_entries = query
                .iter_object_stores(ctx.log_db, obj_tree_props, &[ObjectType::TextEntry])
                .flat_map(|(_obj_type, obj_path, obj_store)| {
                    let mut batch = Vec::new();
                    // TODO(cmc): We're cloning full strings here, which is very much a bad idea.
                    // We need to change the internal storage so that we store ref-counted strings
                    // rather than plain strings.
                    //
                    // On the other hand:
                    // - A) We're about to change our storage engine.
                    // - B) Nobody is logging gazillon of text logs into Rerun yet.
                    visit_type_data_3(
                        obj_store,
                        &FieldName::from("body"),
                        &TimeQuery::EVERYTHING, // always sticky!
                        ("_visible", "level", "color"),
                        |_instance_index: Option<&IndexHash>,
                         time: i64,
                         msg_id: &MsgId,
                         body: &String,
                         visible: Option<&bool>,
                         level: Option<&String>,
                         color: Option<&[u8; 4]>| {
                            if *visible.unwrap_or(&true) {
                                batch.push(TextEntry {
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
                    batch
                });
            self.text_entries.extend(text_entries);
        }
    }
}

impl SceneText {
    pub fn is_empty(&self) -> bool {
        let Self { text_entries } = self;

        text_entries.is_empty()
    }
}
