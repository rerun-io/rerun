//! [`ArrowUi`] can be used to show arbitrary Arrow data with a nice UI.
//! The implementation is inspired from arrows built-in display formatter:
//! https://github.com/apache/arrow-rs/blob/c628435f9f14abc645fb546442132974d3d380ca/arrow-cast/src/display.rs
use crate::datatype_ui::datatype_ui;
use crate::list_item_ranges::list_item_ranges;
use arrow::array::cast::*;
use arrow::array::types::*;
use arrow::array::*;
use arrow::datatypes::{ArrowNativeType, DataType, UnionMode};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use arrow::*;
use egui::text::LayoutJob;
use egui::{Id, RichText, Stroke, StrokeKind, Tooltip, Ui, WidgetText};
use half::f16;
use re_format::{format_f32, format_f64, format_int, format_uint};
use re_ui::UiExt;
use re_ui::list_item::{LabelContent, PropertyContent, list_item_scope};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use std::fmt::{Display, Formatter, Write};
use std::ops::Range;

pub struct ArrayUi<'a> {
    array: &'a dyn Array,
    format: Box<dyn ShowIndex + 'a>,
}

impl<'a> ArrayUi<'a> {
    /// Returns an [`ArrayUi`] that can be used to show `array`
    ///
    /// This returns an error if an array of the given data type cannot be formatted/shown.
    pub fn try_new(array: &'a dyn Array, options: &FormatOptions<'a>) -> Result<Self, ArrowError> {
        Ok(Self {
            array,
            format: make_ui(array, options)?,
        })
    }

    /// Show a single value at `idx`.
    ///
    /// This will create a list item that might have some nested children.
    pub fn show_value(&self, idx: usize, ui: &mut Ui) {
        // TODO: Use ArrowNode or directly call `self.format.show(idx, ui)`?
        // let node = ArrowNode::index(idx, &*self.format);
        // node.show(ui, idx);
        self.format.show(idx, ui);
    }

    /// Show a `list_item` based tree view of the data.
    pub fn show(&self, ui: &mut Ui) {
        list_ui(ui, 0..self.array.len(), &*self.format);
    }

    /// Returns a [`LayoutJob`] that displays (maybe a subset of) the data.
    pub fn job(&self, ui: &Ui) -> Result<LayoutJob, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        write_list(&mut highlighted, 0..self.array.len(), &*self.format)?;
        Ok(highlighted.into_job())
    }

    /// Returns a [`LayoutJob`] that displays a single value at `idx`.
    pub fn value_job(&self, ui: &Ui, idx: usize) -> Result<LayoutJob, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        self.format.write(idx, &mut highlighted)?;
        Ok(highlighted.into_job())
    }
}

fn make_ui<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    downcast_integer_array! {
        array => show_custom(array, options),
        DataType::Float16 => show_custom(array.as_primitive::<Float16Type>(), options),
        DataType::Float32 => show_custom(array.as_primitive::<Float32Type>(), options),
        DataType::Float64 => show_custom(array.as_primitive::<Float64Type>(), options),
        DataType::Null | DataType::Boolean | DataType::Utf8 | DataType::LargeUtf8
        | DataType::Utf8View | DataType::Binary | DataType::BinaryView | DataType::LargeBinary
        | DataType::Date32 | DataType::Date64 | DataType::Time32(_) | DataType::Time64(_)
        | DataType::Timestamp(_, _) | DataType::Duration(_) | DataType::Interval(_)
        | DataType::Decimal128(_, _) | DataType::Decimal256(_, _)
        => {
            show_arrow_builtin(array, options)
        }
        DataType::FixedSizeBinary(_) => {
            let a = array.as_any().downcast_ref::<FixedSizeBinaryArray>().unwrap();
            show_arrow_builtin(a, options)
        }
        DataType::Dictionary(_, _) => downcast_dictionary_array! {
            array => show_custom(array, options),
            _ => unreachable!()
        }
        DataType::List(_) => show_custom(as_generic_list_array::<i32>(array), options),
        DataType::LargeList(_) => show_custom(as_generic_list_array::<i64>(array), options),
        DataType::FixedSizeList(_, _) => {
            let a = array.as_any().downcast_ref::<FixedSizeListArray>().unwrap();
            show_custom(a, options)
        }
        DataType::Struct(_) => show_custom(as_struct_array(array), options),
        DataType::Map(_, _) => show_custom(as_map_array(array), options),
        DataType::Union(_, _) => show_custom(as_union_array(array), options),
        DataType::RunEndEncoded(_, _) => downcast_run_array! {
            array => show_custom(array, options),
            _ => unreachable!()
        },
        DataType::ListView(_) | DataType::LargeListView(_) => {
            Err(ArrowError::NotYetImplemented(
                "ListView and LargeListView are not yet supported".to_string(),
            ))
        }
    }
}

struct ShowBuiltIn<'a> {
    array: &'a dyn Array,
    formatter: ArrayFormatter<'a>,
}

impl<'a> ShowIndex for ShowBuiltIn<'a> {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
        let mut text = String::new();
        self.formatter.value(idx).write(&mut text)?;

        let dt = self.array.data_type();
        if matches!(
            dt,
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
        ) {
            f.code_string_value(&text);
        } else {
            // TODO: Should dates be primitives?
            f.code_primitive(&text);
        }

        Ok(())
    }

    fn array(&self) -> &dyn Array {
        self.array
    }
}

fn show_arrow_builtin<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    Ok(Box::new(ShowBuiltIn {
        formatter: ArrayFormatter::try_new(array, options)?,
        array,
    }))
}

type EmptyArrowResult = Result<(), ArrowError>;

/// [`Display`] but accepting an index
trait ShowIndex {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult;

    fn show(&self, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        let result = self.write(idx, &mut highlighted);
        match result {
            Ok(_) => {
                ui.list_item()
                    .show_hierarchical(ui, LabelContent::new(highlighted.into_widget_text()));
            }
            Err(err) => {
                ui.error_label(format!("Error formatting value: {err:?}"));
            }
        }
    }

    fn is_item_nested(&self) -> bool {
        false
    }

    fn array(&self) -> &dyn Array;
}

/// [`ShowIndex`] with additional state
trait ShowIndexState<'a> {
    type State;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError>;

    fn write(
        &self,
        state: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult;

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        let result = self.write(state, idx, &mut highlighted);
        match result {
            Ok(_) => {
                ui.list_item()
                    .show_hierarchical(ui, LabelContent::new(highlighted.into_widget_text()));
            }
            Err(err) => {
                ui.error_label(format!("Error formatting value: {err:?}"));
            }
        }
    }

    fn is_item_nested(&self) -> bool {
        false
    }
}

fn show_standalone_value(ui: &mut Ui, value: &dyn ShowIndex, idx: usize) {}

impl<'a, T: ShowIndex> ShowIndexState<'a> for T {
    type State = ();

    fn prepare(&self, _options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok(())
    }

    fn write(
        &self,
        _: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        ShowIndex::write(self, idx, f)
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        ShowIndex::show(self, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        ShowIndex::is_item_nested(self)
    }
}

struct ShowCustom<'a, F: ShowIndexState<'a>> {
    state: F::State,
    array: F,
    null: &'a str,
}

fn show_custom<'a, F>(
    array: F,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError>
where
    F: ShowIndexState<'a> + Array + 'a,
{
    let state = array.prepare(options)?;
    Ok(Box::new(ShowCustom {
        state,
        array,
        null: "null",
    }))
}

impl<'a, F: ShowIndexState<'a> + Array> ShowIndex for ShowCustom<'a, F> {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
        if self.array.is_null(idx) {
            if !self.null.is_empty() {
                f.code_primitive(self.null)
            }
            return Ok(());
        }
        ShowIndexState::write(&self.array, &self.state, idx, f)
    }

    fn show(&self, idx: usize, ui: &mut Ui) {
        ShowIndexState::show(&self.array, &self.state, idx, ui);
    }

    fn is_item_nested(&self) -> bool {
        ShowIndexState::is_item_nested(&self.array)
    }

    fn array(&self) -> &dyn Array {
        &self.array
    }
}

macro_rules! primitive_display {
    ($fmt:ident: $($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
                let value = self.value(idx);
                let s = $fmt(value);
                f.code_primitive(&s);
                Ok(())
            }

            fn array(&self) -> &dyn Array {
                self
            }
        })+
    };
}

primitive_display!(format_int: Int8Type, Int16Type, Int32Type, Int64Type);
primitive_display!(format_uint: UInt8Type, UInt16Type, UInt32Type, UInt64Type);
primitive_display!(format_f32: Float32Type);
primitive_display!(format_f64: Float64Type);

// TODO: Is this right? Should we have a format_f16?
fn format_f16(value: f16) -> String {
    format_f32(value.to_f32())
}
primitive_display!(format_f16: Float16Type);

impl<'a, K: ArrowDictionaryKeyType> ShowIndexState<'a> for &'a DictionaryArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let value_idx = self.keys().values()[idx].as_usize();
        s.as_ref().write(value_idx, f)
    }
}

impl<'a, K: RunEndIndexType> ShowIndexState<'a> for &'a RunArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let value_idx = self.get_physical_index(idx);
        s.as_ref().write(value_idx, f)
    }
}

fn write_list(
    f: &mut SyntaxHighlightedBuilder,
    mut range: Range<usize>,
    values: &dyn ShowIndex,
) -> EmptyArrowResult {
    f.code_syntax("[");
    if let Some(idx) = range.next() {
        values.write(idx, f)?;
    }
    for idx in range {
        f.code_syntax(", ");
        values.write(idx, f)?;
    }
    f.code_syntax("]");
    Ok(())
}

enum NodeLabel {
    Index(usize),
    Name(String),
    Custom(WidgetText),
}
pub struct ArrowNode<'a> {
    label: NodeLabel,
    values: &'a dyn ShowIndex,
}

impl<'a> ArrowNode<'a> {
    pub fn custom(name: impl Into<WidgetText>, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Custom(name.into()),
            values,
        }
    }

    pub fn name(name: impl Into<String>, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Name(name.into()),
            values,
        }
    }

    /// The index to *display*
    pub fn index(idx: usize, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Index(idx),
            values,
        }
    }

    /// The index of the *value* to display.
    /// Can be different from [`ArrowNode::index`] e.g. in a sliced array.
    pub fn show(self, ui: &mut Ui, index: usize) {
        let label = match self.label {
            NodeLabel::Index(idx) => {
                let mut builder = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
                builder.code_index(&format_uint(idx));
                builder.into_widget_text()
            }
            NodeLabel::Name(name) => {
                let mut builder = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
                builder.code_name(&name);
                builder.into_widget_text()
            }
            NodeLabel::Custom(name) => name,
        };

        let mut value = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
        self.values.write(index, &mut value); // TODO: Handle error
        let value = value.into_widget_text();

        let nested = self.values.is_item_nested();
        let data_type = self.values.array().data_type();
        let (data_type_name, maybe_datatype_ui) = datatype_ui(data_type);

        let mut item = ui.list_item();
        let id = ui.unique_id().with(index).with(label.text());
        let content = PropertyContent::new(label)
            .value_fn(|ui, visuals| {
                ui.horizontal(|ui| {
                    egui::Sides::new().shrink_left().show(
                        ui,
                        |ui| {
                            if visuals.is_collapsible() && visuals.openness() != 0.0 {
                                if visuals.openness() == 1.0 {
                                    return;
                                }
                                ui.set_opacity(1.0 - visuals.openness());
                            }
                            ui.label(value);
                        },
                        |ui| {
                            let tooltip_open =
                                Tooltip::was_tooltip_open_last_frame(ui.ctx(), ui.next_auto_id());
                            if visuals.hovered || tooltip_open {
                                let response = ui.small(RichText::new(&data_type_name).strong());
                                ui.painter().rect_stroke(
                                    response.rect.expand(2.0),
                                    4.0,
                                    Stroke::new(1.0, visuals.text_color()),
                                    StrokeKind::Middle,
                                );

                                if let Some(content) = maybe_datatype_ui {
                                    response.on_hover_ui(|ui| {
                                        list_item_scope(
                                            ui,
                                            Id::new("arrow data type hover"),
                                            |ui| {
                                                ui.list_item().show_hierarchical_with_children(
                                                    ui,
                                                    Id::new("arrow data type item hover"),
                                                    true,
                                                    LabelContent::new(data_type_name),
                                                    content,
                                                );
                                            },
                                        );
                                    });
                                }
                            }
                        },
                    );
                });
            })
            .show_only_when_collapsed(false);

        if nested {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                self.values.show(index, ui);
            });
        } else {
            item.show_hierarchical(ui, content);
        }
    }
}

/// Show a list.
///
/// If there are enough items, it will show items in a tree of ranges.
/// Since arrow arrays might not start at 0, you can pass a `Range<usize>`.
/// The indexes shown in the UI will be _normalized_ so it's always `0..end-start`
fn list_ui(ui: &mut Ui, mut range: Range<usize>, values: &dyn ShowIndex) {
    let ui_range = 0..(range.end - range.start);

    list_item_ranges(ui, ui_range, &mut |ui, ui_idx| {
        let node = ArrowNode::index(ui_idx, values);
        let array_index = ui_idx + range.start;
        node.show(ui, array_index);
    });
}

impl<'a, O: OffsetSizeTrait> ShowIndexState<'a> for &'a GenericListArray<O> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        write_list(f, start..end, s.as_ref())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        list_ui(ui, start..end, state.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a FixedSizeListArray {
    type State = (usize, Box<dyn ShowIndex + 'a>);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let values = make_ui(self.values().as_ref(), options)?;
        let length = self.value_length();
        Ok((length as usize, values))
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let start = idx * s.0;
        let end = start + s.0;
        write_list(f, start..end, s.1.as_ref())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let start = idx * state.0;
        let end = start + state.0;
        list_ui(ui, start..end, state.1.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        true
    }
}

/// Pairs a boxed [`ShowIndex`] with its field name
type FieldDisplay<'a> = (&'a str, Box<dyn ShowIndex + 'a>);

impl<'a> ShowIndexState<'a> for &'a StructArray {
    type State = Vec<FieldDisplay<'a>>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let fields = match (*self).data_type() {
            DataType::Struct(f) => f,
            _ => unreachable!(),
        };

        self.columns()
            .iter()
            .zip(fields)
            .map(|(a, f)| {
                let format = make_ui(a.as_ref(), options)?;
                Ok((f.name().as_str(), format))
            })
            .collect()
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let mut iter = s.iter();
        f.code_syntax("{");
        if let Some((name, display)) = iter.next() {
            f.code_name(name);
            f.code_syntax(": ");
            display.as_ref().write(idx, f)?;
        }
        for (name, display) in iter {
            f.code_syntax(", ");
            f.code_name(name);
            f.code_syntax(": ");
            display.as_ref().write(idx, f)?;
        }
        f.code_syntax("}");
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        for (name, display) in state {
            let node = ArrowNode::name(*name, display.as_ref());
            node.show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        data_type.is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a MapArray {
    type State = (Box<dyn ShowIndex + 'a>, Box<dyn ShowIndex + 'a>);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let keys = make_ui(self.keys().as_ref(), options)?;
        let values = make_ui(self.values().as_ref(), options)?;
        Ok((keys, values))
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        f.code_syntax("{");
        if let Some(idx) = iter.next() {
            s.0.write(idx, f)?;
            f.code_syntax(": ");
            s.1.write(idx, f)?;
        }

        for idx in iter {
            f.code_syntax(", ");
            s.0.write(idx, f)?;
            f.code_syntax(": ");
            s.1.write(idx, f)?;
        }

        f.code_syntax("}");
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        for idx in iter {
            let mut key_string = SyntaxHighlightedBuilder::new(ui.style(), ui.tokens());
            state.0.write(idx, &mut key_string); // TODO: Handle error

            ArrowNode::custom(key_string.into_widget_text(), state.1.as_ref()).show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        data_type.is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a UnionArray {
    type State = (Vec<Option<(&'a str, Box<dyn ShowIndex + 'a>)>>, UnionMode);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let (fields, mode) = match (*self).data_type() {
            DataType::Union(fields, mode) => (fields, mode),
            _ => unreachable!(),
        };

        let max_id = fields.iter().map(|(id, _)| id).max().unwrap_or_default() as usize;
        let mut out: Vec<Option<FieldDisplay>> = (0..max_id + 1).map(|_| None).collect();
        for (i, field) in fields.iter() {
            let formatter = make_ui(self.child(i).as_ref(), options)?;
            out[i as usize] = Some((field.name().as_str(), formatter))
        }
        Ok((out, *mode))
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let id = self.type_id(idx);
        let idx = match s.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = s.0[id as usize].as_ref().unwrap();

        f.code_syntax("{");
        f.code_name(name);
        f.code_syntax("=");
        field.write(idx, f)?;
        f.code_syntax("}");

        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let id = self.type_id(idx);
        let idx = match state.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = state.0[id as usize].as_ref().unwrap();

        let node = ArrowNode::name(*name, field.as_ref());
        node.show(ui, idx);
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        data_type.is_nested()
    }
}
