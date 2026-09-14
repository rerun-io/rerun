use std::fmt::{Display, Formatter};

use itertools::Itertools as _;

use crate::{DataResultInteractionAddress, Item};

/// Counts items per kind and displays them in a human-readable way, e.g. `2 entities, 1 view`.
///
/// Used wherever a set of items is too large to name individually: the drag-and-drop pill,
/// selection history entries, etc.
#[derive(Debug, Default)]
pub struct ItemCounter {
    container_cnt: u32,
    view_cnt: u32,
    app_cnt: u32,
    table_cnt: u32,
    data_source_cnt: u32,
    store_cnt: u32,
    entity_cnt: u32,
    instance_cnt: u32,
    component_cnt: u32,
    redap_server_cnt: u32,
    redap_entry_cnt: u32,
}

impl ItemCounter {
    pub fn add(&mut self, item: &Item) {
        match item {
            Item::Container(_) => self.container_cnt += 1,
            Item::View(_) => self.view_cnt += 1,
            Item::AppId(_) => self.app_cnt += 1,
            Item::TableId(_) => self.table_cnt += 1,
            Item::DataSource(_) => self.data_source_cnt += 1,
            Item::StoreId(_) => self.store_cnt += 1,
            Item::InstancePath(instance_path)
            | Item::DataResult(DataResultInteractionAddress { instance_path, .. }) => {
                if instance_path.is_all() {
                    self.entity_cnt += 1;
                } else {
                    self.instance_cnt += 1;
                }
            }
            Item::ComponentPath(_) => self.component_cnt += 1,
            Item::RedapServer(_) => self.redap_server_cnt += 1,
            Item::RedapEntry { .. } => self.redap_entry_cnt += 1,
        }
    }
}

impl<'a> FromIterator<&'a Item> for ItemCounter {
    fn from_iter<T: IntoIterator<Item = &'a Item>>(items: T) -> Self {
        let mut counter = Self::default();
        for item in items {
            counter.add(item);
        }
        counter
    }
}

impl Display for ItemCounter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Destructured so that a new counter cannot be forgotten here.
        let Self {
            container_cnt,
            view_cnt,
            app_cnt,
            table_cnt,
            data_source_cnt,
            store_cnt,
            entity_cnt,
            instance_cnt,
            component_cnt,
            redap_server_cnt,
            redap_entry_cnt,
        } = self;

        let count_and_names = [
            (container_cnt, "container", "containers"),
            (view_cnt, "view", "views"),
            (app_cnt, "app", "apps"),
            (table_cnt, "table", "tables"),
            (data_source_cnt, "data source", "data sources"),
            (store_cnt, "store", "stores"),
            (entity_cnt, "entity", "entities"),
            (instance_cnt, "instance", "instances"),
            (component_cnt, "component", "components"),
            (redap_server_cnt, "server", "servers"),
            (redap_entry_cnt, "entry", "entries"),
        ];

        count_and_names
            .into_iter()
            .filter_map(|(&count, name_singular, name_plural)| {
                if count > 0 {
                    Some(format!(
                        "{} {}",
                        re_format::format_uint(count),
                        if count == 1 {
                            name_singular
                        } else {
                            name_plural
                        },
                    ))
                } else {
                    None
                }
            })
            .join(", ")
            .fmt(f)
    }
}
