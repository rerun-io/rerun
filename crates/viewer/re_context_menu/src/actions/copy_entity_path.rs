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
            | Item::View(_)
            | Item::RedapEntry(_)
            | Item::RedapServer(_) => false,
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
        ctx.selection.copy_to_clipboard(ctx.egui_context());
    }
}
