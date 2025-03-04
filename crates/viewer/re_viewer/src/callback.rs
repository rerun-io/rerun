use std::rc::Rc;

use re_viewer_context::Item;
use re_viewer_context::ItemCollection;
use re_viewer_context::ItemContext;

#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum CallbackItem {
    EntityPath { entity_path: String },
}

impl CallbackItem {
    pub fn new(item: &Item, _context: &Option<ItemContext>) -> Option<Self> {
        match item {
            // TODO(jan): output more things
            Item::StoreId(_)
            | Item::AppId(_)
            | Item::View(_)
            | Item::Container(_)
            | Item::ComponentPath(_)
            | Item::DataSource(_) => None,
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
                Some(Self::EntityPath {
                    entity_path: instance_path.entity_path.to_string(),
                })
            }
        }
    }
}

pub struct Callbacks {
    pub on_selection_change: Rc<dyn Fn(Vec<CallbackItem>)>,
    // TODO(jan, andreas): why not add all the stuff from TimelineCallbacks here as well? ;-)
}

impl Callbacks {
    pub fn on_selection_change(&self, items: &ItemCollection) {
        (self.on_selection_change)(
            items
                .iter()
                .filter_map(|(item, context)| CallbackItem::new(item, context))
                .collect(),
        );
    }
}
