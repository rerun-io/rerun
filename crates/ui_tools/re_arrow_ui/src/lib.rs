//! Show arrow data as a tree of rerun `list_items` or as a nicely formatted label with syntax highlighting.

mod arrow_node;
mod arrow_ui;
mod datatype_ui;
mod list_item_ranges;
mod show_index;

pub use arrow_ui::arrow_ui;
pub use datatype_ui::{
    data_type_nesting_depth, data_type_syntax_highlighted, data_type_syntax_highlighted_to_depth,
};
pub use show_index::{ArrayUi, DisplayOptions};

pub fn arrow_syntax_highlighted(
    data: &dyn arrow::array::Array,
) -> Result<re_ui::syntax_highlighting::SyntaxHighlightedBuilder, arrow::error::ArrowError> {
    show_index::ArrayUi::try_new(data, &show_index::DisplayOptions::default())?.highlighted()
}

/// All of `data` as plain text, with nothing left out.
///
/// A single string is returned as-is, without quotes or escapes.
/// A single value of another type is written without the surrounding list brackets.
pub fn arrow_full_text(
    data: &dyn arrow::array::Array,
    timestamp_format: re_format::TimestampFormat,
) -> Result<String, arrow::error::ArrowError> {
    use arrow::array::AsArray as _;

    if data.len() == 1 && data.is_valid(0) {
        match data.data_type() {
            arrow::datatypes::DataType::Utf8 => {
                return Ok(data.as_string::<i32>().value(0).to_owned());
            }
            arrow::datatypes::DataType::LargeUtf8 => {
                return Ok(data.as_string::<i64>().value(0).to_owned());
            }
            arrow::datatypes::DataType::Utf8View => {
                return Ok(data.as_string_view().value(0).to_owned());
            }
            _ => {}
        }
    }

    let array_ui = show_index::ArrayUi::try_new(
        data,
        &show_index::DisplayOptions {
            timestamp_format,
            ..show_index::DisplayOptions::untruncated()
        },
    )?;
    let highlighted = if data.len() == 1 {
        array_ui.value_highlighted(0)?
    } else {
        array_ui.highlighted()?
    };
    Ok(highlighted.text().to_owned())
}
