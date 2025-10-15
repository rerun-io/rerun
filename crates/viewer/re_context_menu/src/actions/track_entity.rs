use re_types::ViewClassIdentifier;
use re_viewer_context::{Item, SystemCommand, SystemCommandSender, ViewId};

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
                let mut asdasd = false;

                if is_3d_view(ctx, view_id)
                    && ctx
                        .viewer_context
                        .recording()
                        .is_logged_entity(&instance_path.entity_path)
                {
                    asdasd = true;
                }

                asdasd
            }
        }
    }

    fn label(&self, _ctx: &ContextMenuContext<'_>) -> String {
        "Track this".to_owned()
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &re_entity_db::InstancePath,
    ) {
        ctx.viewer_context
            .global_context
            .command_sender
            .send_system(SystemCommand::SetTracked(
                *view_id,
                instance_path.entity_path.clone(),
            ));
    }
}

fn is_3d_view(ctx: &ContextMenuContext<'_>, view_id: &ViewId) -> bool {
    let mut hmm = false;

    for asd in ctx
        .viewport_blueprint
        .views
        .iter()
        .filter(|x| x.0 == view_id)
    {
        if asd.1.class_identifier() == ViewClassIdentifier::new("3D") {
            hmm = true
        }
    }
    hmm
}
