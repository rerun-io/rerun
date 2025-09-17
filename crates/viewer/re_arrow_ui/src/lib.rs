//! Show arrow data as a tree of rerun `list_items` or as a nicely formatted label with syntax highlighting.
mod arrow_node;
mod arrow_ui;
mod datatype_ui;
mod list_item_ranges;
mod show_index;

pub use arrow_ui::arrow_ui;

pub fn arrow_syntax_highlighted(
    data: &dyn arrow::array::Array,
) -> Result<re_ui::syntax_highlighting::SyntaxHighlightedBuilder, arrow::error::ArrowError> {
    show_index::ArrayUi::try_new(data, &show_index::DisplayOptions::default())?.highlighted()
}
