use std::rc::Rc;

use re_viewer_context::Item;
use re_viewer_context::ItemCollection;
use re_viewer_context::ItemContext;

///
#[derive(Debug, serde::Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum SelectionItem {
    /// Selected a single entity.
    ///
    /// Examples:
    /// * A full point cloud.
    /// * A mesh.
    Entity { entity_path: String },

    /// Selected a single instance.
    ///
    /// Examples:
    /// * A single point in a point cloud.
    Instance {
        entity_path: String,
        instance_id: u64,
    },

    /// Selected a view.
    View { view_id: String },

    /// Selected a container.
    Container { container_id: String },
}

impl SelectionItem {
    pub fn new(item: &Item, _context: &Option<ItemContext>) -> Option<Self> {
        match item {
            // TODO(jan): output more things
            Item::StoreId(_) | Item::AppId(_) | Item::ComponentPath(_) | Item::DataSource(_) => {
                None
            }
            Item::View(view_id) => Some(Self::View {
                view_id: view_id.uuid().to_string(),
            }),
            Item::Container(container_id) => Some(Self::Container {
                container_id: container_id.uuid().to_string(),
            }),
            Item::InstancePath(instance_path) | Item::DataResult(_, instance_path) => {
                if instance_path.is_all() {
                    Some(Self::Entity {
                        entity_path: instance_path.entity_path.to_string(),
                    })
                } else {
                    Some(Self::Instance {
                        entity_path: instance_path.entity_path.to_string(),
                        instance_id: instance_path.instance.get(),
                    })
                }
            }
        }
    }
}

pub struct Callbacks {
    pub on_selection_change: Rc<dyn Fn(Vec<SelectionItem>)>,
    // TODO(jan, andreas): why not add all the stuff from TimelineCallbacks here as well? ;-)
}

impl Callbacks {
    pub fn on_selection_change(&self, items: &ItemCollection) {
        (self.on_selection_change)(
            items
                .iter()
                .filter_map(|(item, context)| SelectionItem::new(item, context))
                .collect(),
        );
    }
}
