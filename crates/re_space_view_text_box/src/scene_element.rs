use re_arrow_store::LatestAtQuery;
use re_log_types::{component_types, Component};
use re_query::{query_entity_with_primary, QueryError};
use re_viewer_context::{ArchetypeDefinition, SceneElement, SceneQuery, ViewerContext};

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

impl SceneElement for SceneTextBox {
    fn archetype(&self) -> ArchetypeDefinition {
        vec![component_types::TextBox::name()]
    }

    fn populate(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        let store = &ctx.log_db.entity_db.data_store;

        for (ent_path, props) in query.iter_entities() {
            if !props.visible {
                continue;
            }

            let query = LatestAtQuery::new(query.timeline, query.latest_at);
            match query_entity_with_primary::<component_types::TextBox>(
                store,
                &query,
                ent_path,
                &[],
            )
            .and_then(|ent_view| {
                for text_entry in ent_view.iter_primary()?.flatten() {
                    let component_types::TextBox { body } = text_entry;
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
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
