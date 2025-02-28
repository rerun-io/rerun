use itertools::Itertools;
use re_viewer_context::Item;

use crate::{ContextMenuAction, ContextMenuContext};

pub struct CopyEntityPathToClipboard;

impl ContextMenuAction for CopyEntityPathToClipboard {
    fn supports_multi_selection(&self, _ctx: &ContextMenuContext<'_>) -> bool {
        true
    }

    fn supports_item(&self, _ctx: &ContextMenuContext<'_>, item: &Item) -> bool {
        match item {
            Item::AppId(_)
            | Item::DataSource(_)
            | Item::StoreId(_)
            | Item::Container(_)
            | Item::View(_) => false,
            Item::DataResult(..) | Item::InstancePath(_) | Item::ComponentPath(_) => true,
        }
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() == 1 {
            "Copy entity path".to_owned()
        } else {
            "Copy entity paths".to_owned()
        }
    }

    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        let clipboard_text = ctx
            .selection
            .iter()
            .filter_map(|(item, _)| match item {
                Item::AppId(_)
                | Item::DataSource(_)
                | Item::StoreId(_)
                | Item::Container(_)
                | Item::View(_) => None,
                Item::DataResult(_, instance_path) | Item::InstancePath(instance_path) => {
                    Some(instance_path.entity_path.clone())
                }
                Item::ComponentPath(component_path) => Some(component_path.entity_path.clone()),
            })
            .map(|entity_path| entity_path.to_string())
            .join("\n");

        re_log::info!("Copied entity paths to clipboard:\n{}", &clipboard_text);
        ctx.viewer_context.egui_ctx().copy_text(clipboard_text);
    }
}
