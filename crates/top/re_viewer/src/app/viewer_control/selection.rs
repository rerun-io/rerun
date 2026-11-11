use re_entity_db::InstancePath;
use re_protos::viewer_control::v1alpha1::{
    GetSelectionResponse, SelectedComponent, SelectedContainer, SelectedEntity, SelectedItem,
    SelectedRecording, SelectedView, Uuid, selected_item,
};
use re_viewer_context::{Item, StoreHub, ViewId};

use super::super::AppCore;

impl AppCore {
    /// The `get_selection` response: what the user has selected, in selection order.
    pub fn collect_selection(&self, store_hub: &StoreHub) -> GetSelectionResponse {
        let active_recording_id = self.active_recording_id();

        // With blueprint timeline inspection, entities can be selected in the blueprint store,
        // which `store_id` does not name.
        let inspected_blueprint = active_recording_id
            .filter(|_| self.state.app_options().inspect_blueprint_timeline)
            .and_then(|store_id| store_hub.active_blueprint_for_app(store_id.application_id()));
        let is_blueprint_item = |item: &Item| {
            Option::zip(inspected_blueprint, item.entity_path())
                .is_some_and(|(blueprint, entity_path)| blueprint.is_logged_entity(entity_path))
        };

        GetSelectionResponse {
            store_id: active_recording_id.map(|store_id| store_id.to_string()),
            items: self
                .selection_state
                .selected_items()
                .iter_items()
                .filter(|item| !is_blueprint_item(item))
                .filter_map(selected_item)
                .collect(),
        }
    }
}

/// How `get_selection` describes `item`, or `None` for kinds it leaves out.
fn selected_item(item: &Item) -> Option<SelectedItem> {
    let kind = match item {
        Item::InstancePath(instance_path) => {
            selected_item::Kind::Entity(selected_entity(instance_path, None))
        }
        Item::DataResult(data_result) => selected_item::Kind::Entity(selected_entity(
            &data_result.instance_path,
            Some(data_result.view_id),
        )),
        Item::ComponentPath(component_path) => selected_item::Kind::Component(SelectedComponent {
            entity_path: component_path.entity_path.to_string(),
            component: component_path.component.to_string(),
        }),
        Item::View(view_id) => selected_item::Kind::View(SelectedView {
            view_id: Some(uuid(view_id.uuid())),
        }),
        Item::Container(container_id) => selected_item::Kind::Container(SelectedContainer {
            container_id: Some(uuid(container_id.uuid())),
        }),
        Item::StoreId(store_id) => selected_item::Kind::Recording(SelectedRecording {
            store_id: store_id.to_string(),
        }),
        Item::AppId(_)
        | Item::DataSource(_)
        | Item::TableId(_)
        | Item::RedapEntry { .. }
        | Item::RedapServer(_) => return None,
    };
    Some(SelectedItem { kind: Some(kind) })
}

fn selected_entity(instance_path: &InstancePath, view_id: Option<ViewId>) -> SelectedEntity {
    SelectedEntity {
        entity_path: instance_path.entity_path.to_string(),
        instance: instance_path
            .instance
            .specific_index()
            .map(|instance| instance.get()),
        view_id: view_id.map(|view_id| uuid(view_id.uuid())),
    }
}

fn uuid(uuid: uuid::Uuid) -> Uuid {
    Uuid {
        uuid: uuid.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::{EntityPath, StoreId, StoreKind};
    use re_viewer_context::DataResultInteractionAddress;

    use super::*;

    #[test]
    fn an_instance_selected_in_a_view_keeps_index_and_view() {
        let view_id = ViewId::random();
        let item = Item::DataResult(DataResultInteractionAddress {
            view_id,
            instance_path: InstancePath::instance(EntityPath::from("/world/points"), 7),
            visualizer: None,
        });

        assert_eq!(
            selected_item(&item).and_then(|item| item.kind),
            Some(selected_item::Kind::Entity(SelectedEntity {
                entity_path: "/world/points".to_owned(),
                instance: Some(7),
                view_id: Some(uuid(view_id.uuid())),
            }))
        );
        assert_eq!(selected_item(&Item::AppId("my_app".into())), None);
    }

    #[cfg(feature = "testing")]
    #[tokio::test]
    async fn reports_the_selection_in_order_with_the_active_recording() {
        use re_viewer_context::{ItemCollection, Route, SystemCommand, SystemCommandSender as _};

        let mut harness = crate::viewer_test_utils::viewer_harness(&Default::default());
        let egui_ctx = harness.ctx.clone();
        let crate::App {
            store_hub,
            core: app,
        } = harness.state_mut();
        let store_id = StoreId::random(StoreKind::Recording, "test_app");
        store_hub.entity_db_entry(&store_id);
        let view_id = ViewId::random();

        app.command_sender
            .send_system(SystemCommand::SetRoute(Route::LocalRecording {
                recording_id: store_id.clone(),
            }));
        app.command_sender.send_system(SystemCommand::set_selection(
            ItemCollection::from_items_and_context([
                (Item::View(view_id), None),
                (
                    Item::InstancePath(EntityPath::from("/world/points").into()),
                    None,
                ),
            ]),
        ));
        app.run_pending_system_commands(store_hub, &egui_ctx);

        let selection = app.collect_selection(store_hub);
        assert_eq!(selection.store_id, Some(store_id.to_string()));
        assert_eq!(
            selection
                .items
                .into_iter()
                .filter_map(|item| item.kind)
                .collect::<Vec<_>>(),
            vec![
                selected_item::Kind::View(SelectedView {
                    view_id: Some(uuid(view_id.uuid())),
                }),
                selected_item::Kind::Entity(SelectedEntity {
                    entity_path: "/world/points".to_owned(),
                    instance: None,
                    view_id: None,
                }),
            ]
        );
    }
}
