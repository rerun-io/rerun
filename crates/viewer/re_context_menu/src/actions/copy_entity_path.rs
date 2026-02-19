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
            | Item::TableId(_)
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
        let mut components = false;
        let mut entities = false;

        for item in ctx.selection.iter_items() {
            match item {
                Item::ComponentPath(_) => components = true,
                Item::InstancePath(_) | Item::DataResult(_) => entities = true,
                _ => {}
            }
        }

        let descriptor = match (components, entities) {
            (true, true) | (false, false) => "",
            (true, false) => "component ",
            (false, true) => "entity ",
        };

        let s = if ctx.selection.len() == 1 { "" } else { "s" };

        format!("Copy {descriptor}path{s}")
    }

    fn process_selection(&self, ctx: &ContextMenuContext<'_>) {
        ctx.selection.copy_to_clipboard(ctx.egui_context());
    }
}
