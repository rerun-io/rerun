use re_sdk_types::ViewClassIdentifier;
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_viewer_context::{Item, ViewId};
use re_viewport_blueprint::ViewProperty;

use crate::{ContextMenuAction, ContextMenuContext};

pub struct TrackEntity;

impl ContextMenuAction for TrackEntity {
    fn supports_item(&self, ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::AppId(_)
            | Item::TableId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::Container(_)
            | Item::View(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_)
            | Item::ComponentPath(_)
            | Item::InstancePath(_) => false,
            Item::DataResult(view_id, instance_path) => {
                is_3d_view(ctx, view_id)
                    // We need to check if the focused entity is logged
                    // because entities without any data don't have bounding boxes or positions.
                    && ctx
                        .viewer_context
                        .recording()
                        .is_logged_entity(&instance_path.entity_path)
            }
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Set as eye tracked".to_owned()
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &re_entity_db::InstancePath,
    ) {
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.viewer_context.blueprint_db(),
            ctx.viewer_context.blueprint_query,
            *view_id,
        );

        eye_property.save_blueprint_component(
            ctx.viewer_context,
            &EyeControls3D::descriptor_tracking_entity(),
            &re_sdk_types::components::EntityPath::from(&instance_path.entity_path),
        );
    }
}

fn is_3d_view(ctx: &ContextMenuContext<'_>, view_id: &ViewId) -> bool {
    ctx.viewport_blueprint
        .views
        .iter()
        .filter(|view| view.0 == view_id)
        .any(|current_view| current_view.1.class_identifier() == ViewClassIdentifier::new("3D"))
}
