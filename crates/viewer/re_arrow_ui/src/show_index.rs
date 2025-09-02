//! [`ArrayUi`] can be used to show arbitrary Arrow data with a nice UI.
//! The implementation is inspired from arrows built-in display formatter:
//! <https://github.com/apache/arrow-rs/blob/c628435f9f14abc645fb546442132974d3d380ca/arrow-cast/src/display.rs>
use crate::arrow_node::ArrowNode;
use crate::list_item_ranges::list_item_ranges;
use arrow::array::cast::{
    AsArray as _, as_generic_list_array, as_map_array, as_struct_array, as_union_array,
};
use arrow::array::types::{
    ArrowDictionaryKeyType, Float16Type, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type,
    Int64Type, RunEndIndexType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow::array::{
    Array, ArrayAccessor as _, DictionaryArray, FixedSizeBinaryArray, FixedSizeListArray,
    GenericBinaryArray, GenericListArray, MapArray, OffsetSizeTrait, PrimitiveArray, RunArray,
    StructArray, UnionArray, as_generic_binary_array, downcast_dictionary_array,
    downcast_integer_array, downcast_run_array,
};
use arrow::datatypes::{ArrowNativeType as _, DataType, Field, UnionMode};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use egui::text::LayoutJob;
use egui::{RichText, Ui};
use re_ui::UiExt as _;
use re_ui::list_item::LabelContent;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use std::ops::Range;

/// The maximum number of items when formatting an array to string.
///
/// If an array has more items, it will be truncated with `…`.
pub const MAX_ARROW_LIST_ITEMS: usize = 10;

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
    /// The list item will _not_ display the index.
    pub fn show_value(&self, idx: usize, ui: &mut Ui) {
        self.format.show(idx, ui);
    }

    /// Show a `list_item` based tree view of the data.
    pub fn show(&self, ui: &mut Ui) {
        list_ui(ui, 0..self.array.len(), &*self.format);
    }

    /// Returns a [`LayoutJob`] that displays the data.
    ///
    /// Arrays will be limited to a sane number of items ([`MAX_ARROW_LIST_ITEMS`]).
    pub fn job(&self, ui: &Ui) -> Result<LayoutJob, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style());
        write_list(&mut highlighted, 0..self.array.len(), &*self.format)?;
        Ok(highlighted.into_job())
    }

    /// Returns a [`LayoutJob`] that displays a single value at `idx`.
    ///
    /// Nested arrays will be limited to a sane number of items ([`MAX_ARROW_LIST_ITEMS`]).
    pub fn value_job(&self, ui: &Ui, idx: usize) -> Result<LayoutJob, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style());
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
        | DataType::Utf8View | DataType::BinaryView
        | DataType::Date32 | DataType::Date64 | DataType::Time32(_) | DataType::Time64(_)
        | DataType::Timestamp(_, _) | DataType::Duration(_) | DataType::Interval(_)
        | DataType::Decimal128(_, _) | DataType::Decimal256(_, _)
        => {
            show_arrow_builtin(array, options)
        }
        DataType::FixedSizeBinary(_) => {
            let a = array.as_any().downcast_ref::<FixedSizeBinaryArray>().expect("FixedSizeBinaryArray downcast failed");
            show_arrow_builtin(a, options)
        }
        DataType::Binary => {
            show_custom(as_generic_binary_array::<i32>(array), options)
        }
        DataType::LargeBinary => {
            show_custom(as_generic_binary_array::<i64>(array), options)
        }
        DataType::Dictionary(_, _) => downcast_dictionary_array! {
            array => show_custom(array, options),
            _ => unreachable!()
        }
        DataType::List(_) => show_custom(as_generic_list_array::<i32>(array), options),
        DataType::LargeList(_) => show_custom(as_generic_list_array::<i64>(array), options),
        DataType::FixedSizeList(_, _) => {
            let a = array.as_any().downcast_ref::<FixedSizeListArray>().expect("FixedSizeListArray downcast failed");
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
                "ListView and LargeListView are not yet supported".to_owned(),
            ))
        }
    }
}

struct ShowBuiltIn<'a> {
    array: &'a dyn Array,
    formatter: ArrayFormatter<'a>,
}

impl ShowIndex for ShowBuiltIn<'_> {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder<'_>) -> EmptyArrowResult {
        let mut text = String::new();
        self.formatter.value(idx).write(&mut text)?;

        let dt = self.array.data_type();

        if self.array.is_null(idx) {
            f.code_primitive("null");
        } else if matches!(
            dt,
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
        ) {
            f.code_string_value(&text);
        } else {
            f.code_primitive(&text);
        }

        Ok(())
    }

    fn array(&self) -> &dyn Array {
        self.array
    }
}

/// Show an array using arrows built-in display formatter.
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

/// UI-equivalent of arrows `DisplayIndex` trait.
pub(crate) trait ShowIndex {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder<'_>) -> EmptyArrowResult;

    fn show(&self, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style());
        let result = self.write(idx, &mut highlighted);
        match result {
            Ok(()) => {
                ui.list_item()
                    .show_hierarchical(ui, LabelContent::new(highlighted.into_widget_text()));
            }
            Err(err) => {
                ui.error_label(err.to_string());
            }
        }
    }

    /// Is a single item of this array nested?
    ///
    /// If true, the list item will be shown as an expandable node.
    fn is_item_nested(&self) -> bool {
        false
    }

    fn array(&self) -> &dyn Array;
}

/// [`ShowIndex`] with additional state.
///
/// UI-equivalent of arrows `DisplayIndexState` trait.
trait ShowIndexState<'a> {
    type State;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError>;

    fn write(
        &self,
        state: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult;

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new(ui.style());
        let result = self.write(state, idx, &mut highlighted);
        match result {
            Ok(()) => {
                ui.list_item()
                    .show_hierarchical(ui, LabelContent::new(highlighted.into_widget_text()));
            }
            Err(err) => {
                ui.error_label(err.to_string());
            }
        }
    }

    fn is_item_nested(&self) -> bool {
        false
    }
}

impl<'a, T: ShowIndex> ShowIndexState<'a> for T {
    type State = ();

    fn prepare(&self, _options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        Ok(())
    }

    fn write(
        &self,
        _: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult {
        ShowIndex::write(self, idx, f)
    }

    fn show(&self, _state: &Self::State, idx: usize, ui: &mut Ui) {
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
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder<'_>) -> EmptyArrowResult {
        if self.array.is_null(idx) {
            if !self.null.is_empty() {
                f.code_primitive(self.null);
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
    ($fmt:path: $($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder<'_>) -> EmptyArrowResult {
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

primitive_display!(re_format::format_int: Int8Type, Int16Type, Int32Type, Int64Type);
primitive_display!(re_format::format_uint: UInt8Type, UInt16Type, UInt32Type, UInt64Type);
primitive_display!(re_format::format_f32: Float32Type);
primitive_display!(re_format::format_f64: Float64Type);
primitive_display!(re_format::format_f16: Float16Type);

impl<OffsetSize: OffsetSizeTrait> ShowIndex for &GenericBinaryArray<OffsetSize> {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder<'_>) -> EmptyArrowResult {
        let value = self.value(idx);
        f.code_primitive(&re_format::format_bytes(value.len() as f64));
        Ok(())
    }

    fn array(&self) -> &dyn Array {
        self
    }
}

impl<'a, K: ArrowDictionaryKeyType> ShowIndexState<'a> for &'a DictionaryArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
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
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult {
        let value_idx = self.get_physical_index(idx);
        s.as_ref().write(value_idx, f)
    }
}

fn write_list(
    f: &mut SyntaxHighlightedBuilder<'_>,
    mut range: Range<usize>,
    values: &dyn ShowIndex,
) -> EmptyArrowResult {
    f.code_syntax("[");
    if let Some(idx) = range.next() {
        values.write(idx, f)?;
    }

    let mut items = 1;

    for idx in range {
        if items >= MAX_ARROW_LIST_ITEMS {
            f.code_syntax(", …");
            break;
        }
        f.code_syntax(", ");
        values.write(idx, f)?;
        items += 1;
    }
    f.code_syntax("]");
    Ok(())
}

/// Show a list.
///
/// If there are enough items, it will show items in a tree of ranges.
///
/// Since arrow arrays might not start at 0, you need pass a `Range<usize>`.
/// E.g. a [`GenericListArray`] consists of a single large values array and an offsets array.
/// So the nth list would be a slice of the main array based on the offsets array at n.
/// See the [`GenericListArray`] docs for more info.
///
/// The indexes shown in the UI will be _normalized_ so it's always `0..end-start`.
pub(crate) fn list_ui(ui: &mut Ui, range: Range<usize>, values: &dyn ShowIndex) {
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
        f: &mut SyntaxHighlightedBuilder<'_>,
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
        f: &mut SyntaxHighlightedBuilder<'_>,
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
type FieldDisplay<'a> = (&'a Field, Box<dyn ShowIndex + 'a>);

impl<'a> ShowIndexState<'a> for &'a StructArray {
    type State = Vec<FieldDisplay<'a>>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let fields = self.fields();

        self.columns()
            .iter()
            .zip(fields)
            .map(|(a, f)| {
                let format = make_ui(a.as_ref(), options)?;
                Ok((&**f, format))
            })
            .collect()
    }

    fn write(
        &self,
        s: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult {
        let mut iter = s.iter();
        f.code_syntax("{");
        if let Some((field, display)) = iter.next() {
            f.code_identifier(field.name());
            f.code_syntax(": ");
            display.as_ref().write(idx, f)?;
        }
        for (field, display) in iter {
            f.code_syntax(", ");
            f.code_identifier(field.name());
            f.code_syntax(": ");
            display.as_ref().write(idx, f)?;
        }
        f.code_syntax("}");
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        for (field, show_field) in state {
            let node = ArrowNode::field(field, show_field.as_ref());
            node.show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
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
        (keys, values): &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        f.code_syntax("{");
        if let Some(idx) = iter.next() {
            keys.write(idx, f)?;
            f.code_syntax(": ");
            values.write(idx, f)?;
        }

        for idx in iter {
            f.code_syntax(", ");
            keys.write(idx, f)?;
            f.code_syntax(": ");
            values.write(idx, f)?;
        }

        f.code_syntax("}");
        Ok(())
    }

    fn show(&self, (keys, values): &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let iter = start..end;

        for idx in iter {
            let mut key_string = SyntaxHighlightedBuilder::new(ui.style());
            let result = keys.write(idx, &mut key_string);
            let text = if result.is_err() {
                RichText::new("cannot display key")
                    .color(ui.tokens().error_fg_color)
                    .into()
            } else {
                key_string.into_widget_text()
            };

            ArrowNode::custom(text, values.as_ref()).show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
    }
}

impl<'a> ShowIndexState<'a> for &'a UnionArray {
    type State = (Vec<Option<FieldDisplay<'a>>>, UnionMode);

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let DataType::Union(fields, mode) = (*self).data_type() else {
            unreachable!()
        };

        let max_id = fields.iter().map(|(id, _)| id).max().unwrap_or_default() as usize;
        let mut show_fields: Vec<Option<FieldDisplay<'_>>> =
            (0..max_id + 1).map(|_| None).collect();
        for (i, field) in fields.iter() {
            let formatter = make_ui(self.child(i).as_ref(), options)?;
            show_fields[i as usize] = Some((field, formatter));
        }
        Ok((show_fields, *mode))
    }

    fn write(
        &self,
        (fields, mode): &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder<'_>,
    ) -> EmptyArrowResult {
        let id = self.type_id(idx);
        let idx = match mode {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (field, show_field) = fields[id as usize]
            .as_ref()
            .expect("Union field should be present");

        f.code_syntax("{");
        f.code_identifier(field.name());
        f.code_syntax("=");
        show_field.write(idx, f)?;
        f.code_syntax("}");

        Ok(())
    }

    fn show(&self, (fields, mode): &Self::State, idx: usize, ui: &mut Ui) {
        let id = self.type_id(idx);
        let idx = match mode {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (field, show_field) = fields[id as usize]
            .as_ref()
            .expect("Union field should be present");

        let node = ArrowNode::field(field, show_field.as_ref());
        node.show(ui, idx);
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
    }
}
