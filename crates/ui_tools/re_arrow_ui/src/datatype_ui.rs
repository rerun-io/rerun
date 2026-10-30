use arrow::datatypes::{DataType, Field};
use re_ui::list_item::PropertyContent;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{HasDesignTokens as _, UiExt as _};

/// A data type on a single line, written the way arrow-rs prints it, with type names, numbers and field names colored.
///
/// Leaves out list field names and field metadata.
pub fn data_type_syntax_highlighted(
    data_type: &DataType,
    style: &egui::Style,
) -> egui::text::LayoutJob {
    data_type_syntax_highlighted_to_depth(data_type, usize::MAX, style)
}

/// Like [`data_type_syntax_highlighted`], but writes the content of nested types as `…` below `max_depth` levels of nesting.
///
/// With a `max_depth` of 0, `List(non-null UInt8)` comes out as `List(…)`.
/// Parameters of non-nested types, like the unit of a `Timestamp`, always show.
pub fn data_type_syntax_highlighted_to_depth(
    data_type: &DataType,
    max_depth: usize,
    style: &egui::Style,
) -> egui::text::LayoutJob {
    let mut highlighter = DataTypeHighlighter {
        builder: SyntaxHighlightedBuilder::new(),
        type_name: egui::TextFormat {
            font_id: egui::TextStyle::Monospace.resolve(style),
            color: style.tokens().code_keyword_color,
            ..Default::default()
        },
        depth: 0,
        max_depth,
    };

    highlighter.data_type(data_type);
    highlighter.builder.to_job(style)
}

/// The number of levels of nesting in `data_type`.
///
/// This is the lowest `max_depth` at which [`data_type_syntax_highlighted_to_depth`] writes the whole data type.
pub fn data_type_nesting_depth(data_type: &DataType) -> usize {
    let deepest = |data_types: &mut dyn Iterator<Item = &DataType>| {
        1 + data_types.map(data_type_nesting_depth).max().unwrap_or(0)
    };

    match data_type {
        DataType::List(field)
        | DataType::ListView(field)
        | DataType::LargeList(field)
        | DataType::LargeListView(field)
        | DataType::FixedSizeList(field, _)
        | DataType::Map(field, _) => deepest(&mut std::iter::once(field.data_type())),
        DataType::Struct(fields) => deepest(&mut fields.iter().map(|field| field.data_type())),
        DataType::Union(fields, _) => {
            deepest(&mut fields.iter().map(|(_, field)| field.data_type()))
        }
        DataType::Dictionary(key, value) => {
            deepest(&mut [key.as_ref(), value.as_ref()].into_iter())
        }
        DataType::RunEndEncoded(run_ends, values) => {
            deepest(&mut [run_ends.data_type(), values.data_type()].into_iter())
        }
        _ => 0,
    }
}

struct DataTypeHighlighter {
    builder: SyntaxHighlightedBuilder,

    /// The format of type names, monospace in the keyword color.
    type_name: egui::TextFormat,

    /// The number of nested types the current one is inside of.
    depth: usize,

    /// Nested types at this depth show their content as `…`.
    max_depth: usize,
}

impl DataTypeHighlighter {
    fn data_type(&mut self, data_type: &DataType) {
        match data_type {
            DataType::List(field) => self.nested("List", |h| h.field(field)),
            DataType::ListView(field) => self.nested("ListView", |h| h.field(field)),
            DataType::LargeList(field) => self.nested("LargeList", |h| h.field(field)),
            DataType::LargeListView(field) => self.nested("LargeListView", |h| h.field(field)),
            DataType::FixedSizeList(field, size) => self.nested("FixedSizeList", |h| {
                h.builder
                    .append_primitive(&size.to_string())
                    .append_syntax(" x ");
                h.field(field);
            }),
            DataType::Struct(fields) => self.nested("Struct", |h| {
                h.named_fields(fields.iter().map(|field| field.as_ref()));
            }),
            DataType::Union(fields, mode) => self.nested("Union", |h| {
                h.builder.append_primitive(&format!("{mode:?}"));
                for (type_id, field) in fields.iter() {
                    h.builder
                        .append_syntax(", ")
                        .append_primitive(&type_id.to_string())
                        .append_syntax(": (");
                    h.named_field(field);
                    h.builder.append_syntax(")");
                }
            }),
            DataType::Map(field, sorted) => self.nested("Map", |h| {
                h.named_field(field);
                h.builder.append_syntax(", ").append_primitive(if *sorted {
                    "sorted"
                } else {
                    "unsorted"
                });
            }),
            DataType::Dictionary(key, value) => self.nested("Dictionary", |h| {
                h.data_type(key);
                h.builder.append_syntax(", ");
                h.data_type(value);
            }),
            DataType::RunEndEncoded(run_ends, values) => self.nested("RunEndEncoded", |h| {
                h.named_fields([run_ends.as_ref(), values.as_ref()]);
            }),
            DataType::Timestamp(unit, timezone) => self.brackets("Timestamp", |h| {
                h.builder.append_primitive(&unit.to_string());
                if let Some(timezone) = timezone {
                    h.builder.append_syntax(", ").append_string_value(timezone);
                }
            }),
            DataType::Time32(unit) => self.params("Time32", &[&unit.to_string()]),
            DataType::Time64(unit) => self.params("Time64", &[&unit.to_string()]),
            DataType::Duration(unit) => self.params("Duration", &[&unit.to_string()]),
            DataType::Interval(unit) => self.params("Interval", &[&format!("{unit:?}")]),
            DataType::FixedSizeBinary(size) => {
                self.params("FixedSizeBinary", &[&size.to_string()]);
            }
            DataType::Decimal32(precision, scale) => {
                self.params("Decimal32", &[&precision.to_string(), &scale.to_string()]);
            }
            DataType::Decimal64(precision, scale) => {
                self.params("Decimal64", &[&precision.to_string(), &scale.to_string()]);
            }
            DataType::Decimal128(precision, scale) => {
                self.params("Decimal128", &[&precision.to_string(), &scale.to_string()]);
            }
            DataType::Decimal256(precision, scale) => {
                self.params("Decimal256", &[&precision.to_string(), &scale.to_string()]);
            }
            DataType::Null
            | DataType::Boolean
            | DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float16
            | DataType::Float32
            | DataType::Float64
            | DataType::Date32
            | DataType::Date64
            | DataType::Binary
            | DataType::LargeBinary
            | DataType::BinaryView
            | DataType::Utf8
            | DataType::LargeUtf8
            | DataType::Utf8View => {
                self.builder
                    .append_with_format(&data_type.to_string(), self.type_name.clone());
            }
        }
    }

    /// The type name followed by `content` between brackets.
    ///
    /// At `max_depth`, a `…` takes the place of `content`.
    fn nested(&mut self, name: &str, content: impl FnOnce(&mut Self)) {
        self.brackets(name, |h| {
            if h.depth >= h.max_depth {
                h.builder.append_syntax("…");
            } else {
                h.depth += 1;
                content(h);
                h.depth -= 1;
            }
        });
    }

    /// The type name followed by `content` between brackets, at any depth.
    fn brackets(&mut self, name: &str, content: impl FnOnce(&mut Self)) {
        self.builder
            .append_with_format(name, self.type_name.clone())
            .append_syntax("(");
        content(self);
        self.builder.append_syntax(")");
    }

    /// `name(a, b)` with primitive parameters.
    fn params(&mut self, name: &str, params: &[&str]) {
        self.brackets(name, |h| {
            for (index, param) in params.iter().enumerate() {
                if index > 0 {
                    h.builder.append_syntax(", ");
                }
                h.builder.append_primitive(param);
            }
        });
    }

    fn field(&mut self, field: &Field) {
        if !field.is_nullable() {
            // This follows the notation set by arrow-rs.
            self.builder.append_syntax("non-null ");
        }

        self.data_type(field.data_type());
    }

    fn named_field(&mut self, field: &Field) {
        self.builder
            .append_string_value(field.name())
            .append_syntax(": ");
        self.field(field);
    }

    fn named_fields<'a>(&mut self, fields: impl IntoIterator<Item = &'a Field>) {
        for (index, field) in fields.into_iter().enumerate() {
            if index > 0 {
                self.builder.append_syntax(", ");
            }
            self.named_field(field);
        }
    }
}

/// Show an ui describing an Arrow `DataType`.
///
/// This is basically a string type name and an optional closure describing nested fields.
pub struct DataTypeUi<'a> {
    /// A short name for the type, e.g. "struct", "list", "int32", etc.
    ///
    /// Doesn't contain info about nested fields.
    pub type_name: String,

    /// A closure for showing a list item with info about nested fields.
    ///
    /// Only set if the type actually has nested fields.
    #[expect(clippy::type_complexity)]
    pub content: Option<Box<dyn FnOnce(&mut egui::Ui) + 'a>>,
}

impl<'a> DataTypeUi<'a> {
    pub fn new(data_type: &'a DataType) -> Self {
        match data_type {
            DataType::Struct(fields) => DataTypeUi {
                type_name: "Struct".to_owned(),
                content: Some(Box::new(move |ui| {
                    for field in fields {
                        data_type_field_ui(ui, field);
                    }
                })),
            },
            DataType::List(field) => DataTypeUi {
                type_name: "List".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::ListView(field) => DataTypeUi {
                type_name: "ListView".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::FixedSizeList(field, size) => DataTypeUi {
                type_name: format!("FixedSizeList({size})"),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::LargeList(field) => DataTypeUi {
                type_name: "LargeList".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::LargeListView(field) => DataTypeUi {
                type_name: "LargeListView".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, field);
                })),
            },
            DataType::Union(fields, mode) => {
                let label = match mode {
                    arrow::datatypes::UnionMode::Sparse => "Union(Sparse)",
                    arrow::datatypes::UnionMode::Dense => "Union(Dense)",
                };
                DataTypeUi {
                    type_name: label.to_owned(),
                    content: Some(Box::new(move |ui| {
                        for (_, field) in fields.iter() {
                            data_type_field_ui(ui, field);
                        }
                    })),
                }
            }
            DataType::Dictionary(_k, _v) => DataTypeUi {
                type_name: "Dictionary".to_owned(),
                content: None,
            },
            DataType::Map(a, _) => DataTypeUi {
                type_name: "Map".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, a);
                })),
            },
            DataType::RunEndEncoded(a, b) => DataTypeUi {
                type_name: "RunEndEncoded".to_owned(),
                content: Some(Box::new(move |ui| {
                    data_type_field_ui(ui, a);
                    data_type_field_ui(ui, b);
                })),
            },
            non_nested => {
                let label = simple_data_type_string(non_nested)
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| non_nested.to_string());
                DataTypeUi {
                    type_name: label,
                    content: None,
                }
            }
        }
    }

    /// Show the data type as a `list_item`.
    ///
    /// The root item has the label "Data type".
    pub fn list_item_ui(self, ui: &mut egui::Ui) {
        let content = PropertyContent::new("Data type")
            .value_text(self.type_name)
            .show_only_when_collapsed(false);
        if let Some(datatype_ui) = self.content {
            ui.list_item().show_hierarchical_with_children(
                ui,
                ui.make_persistent_id("data_type_ui_root"),
                false,
                content,
                datatype_ui,
            );
        } else {
            ui.list_item().show_hierarchical(ui, content);
        }
    }
}

fn data_type_field_ui(ui: &mut egui::Ui, field: &arrow::datatypes::Field) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let item = ui.list_item();

    let data_type_ui = DataTypeUi::new(field.data_type());

    let text = if field.is_nullable() {
        data_type_ui.type_name.clone()
    } else {
        // This follows the notation set by arrow-rs.
        // If we change this, we should probably change
        // arrow-rs and datafusion to match.
        format!("non-null {}", data_type_ui.type_name)
    };

    let property = PropertyContent::new(field.name())
        .value_text(text)
        .show_only_when_collapsed(false);

    if let Some(content) = data_type_ui.content {
        item.show_hierarchical_with_children(
            ui,
            ui.unique_id().with(field.name()),
            true,
            property,
            content,
        );
    } else {
        item.show_hierarchical(ui, property);
    }
}

// TODO(#11071): there is some overlap here with `re_arrow_util::format` and `codegen`.
pub(crate) fn simple_data_type_string(datatype: &DataType) -> Option<&'static str> {
    match datatype {
        DataType::Null => Some("Null"),
        DataType::Boolean => Some("Boolean"),
        DataType::Int8 => Some("Int8"),
        DataType::Int16 => Some("Int16"),
        DataType::Int32 => Some("Int32"),
        DataType::Int64 => Some("Int64"),
        DataType::UInt8 => Some("UInt8"),
        DataType::UInt16 => Some("UInt16"),
        DataType::UInt32 => Some("UInt32"),
        DataType::UInt64 => Some("UInt64"),
        DataType::Float16 => Some("Float16"),
        DataType::Float32 => Some("Float32"),
        DataType::Float64 => Some("Float64"),
        DataType::Utf8 | DataType::LargeUtf8 => Some("Utf8"),
        DataType::Binary | DataType::LargeBinary => Some("Binary"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::datatypes::{TimeUnit, UnionFields, UnionMode};

    use super::*;

    /// The highlighted text of nested data types reads the same as arrow-rs prints them.
    #[test]
    fn syntax_highlighted_matches_arrow_display() {
        let entries = Field::new(
            "entries",
            DataType::Struct(
                vec![
                    Field::new("key", DataType::Utf8, false),
                    Field::new("value", DataType::Float64, true),
                ]
                .into(),
            ),
            false,
        );
        let union = UnionFields::try_new(
            [0, 1],
            [
                Field::new("a", DataType::Int32, true),
                Field::new("b", DataType::Utf8, true),
            ],
        )
        .unwrap();

        for data_type in [
            DataType::List(Arc::new(Field::new("item", DataType::Float32, false))),
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::UInt8, true)), 3),
            DataType::Map(Arc::new(entries), true),
            DataType::Union(union, UnionMode::Sparse),
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(DataType::Utf8)),
            DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
            DataType::Decimal128(10, 2),
        ] {
            let job = data_type_syntax_highlighted(&data_type, &egui::Style::default());
            assert_eq!(job.text, data_type.to_string());
        }
    }

    /// Nested types below the depth limit show their content as `…`, while parameters of other types always show.
    #[test]
    fn syntax_highlighted_to_depth() {
        let style = egui::Style::default();
        let list = DataType::List(Arc::new(Field::new("item", DataType::UInt8, false)));
        let data_type = DataType::Struct(
            vec![
                Field::new("a", list, true),
                Field::new(
                    "b",
                    DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                    true,
                ),
            ]
            .into(),
        );

        let text =
            |max_depth| data_type_syntax_highlighted_to_depth(&data_type, max_depth, &style).text;
        assert_eq!(text(0), "Struct(…)");
        assert_eq!(
            text(1),
            "Struct(\"a\": List(…), \"b\": Timestamp(ns, \"UTC\"))"
        );
        assert_eq!(text(2), data_type.to_string());
    }

    /// The nesting depth is the lowest depth limit that writes the whole data type.
    #[test]
    fn nesting_depth_matches_highlighter() {
        let style = egui::Style::default();
        let list = |data_type| DataType::List(Arc::new(Field::new("item", data_type, true)));
        let union = UnionFields::try_new(
            [0, 1],
            [
                Field::new("a", list(DataType::Int32), true),
                Field::new("b", DataType::Utf8, true),
            ],
        )
        .unwrap();

        for data_type in [
            DataType::Float32,
            DataType::Timestamp(TimeUnit::Nanosecond, None),
            list(DataType::UInt8),
            list(list(DataType::UInt8)),
            DataType::Struct(
                vec![
                    Field::new("a", list(DataType::Float64), true),
                    Field::new("b", DataType::Int8, true),
                ]
                .into(),
            ),
            DataType::Union(union, UnionMode::Dense),
            DataType::Dictionary(Box::new(DataType::Int16), Box::new(list(DataType::Utf8))),
        ] {
            let depth = data_type_nesting_depth(&data_type);
            let text = |max_depth| {
                data_type_syntax_highlighted_to_depth(&data_type, max_depth, &style).text
            };
            assert_eq!(text(depth), data_type.to_string());
            if let Some(shallower) = depth.checked_sub(1) {
                assert_ne!(text(shallower), data_type.to_string());
            }
        }
    }
}
