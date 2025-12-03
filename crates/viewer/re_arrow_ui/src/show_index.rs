//! [`ArrayUi`] can be used to show arbitrary Arrow data with a nice UI.
//! The implementation is inspired from arrows built-in display formatter:
//! <https://github.com/apache/arrow-rs/blob/c628435f9f14abc645fb546442132974d3d380ca/arrow-cast/src/display.rs>
use std::ops::Range;

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
    StructArray, TimestampMicrosecondArray, TimestampMillisecondArray, TimestampNanosecondArray,
    TimestampSecondArray, UnionArray, as_generic_binary_array, downcast_dictionary_array,
    downcast_integer_array, downcast_run_array,
};
use arrow::datatypes::{
    ArrowNativeType as _, DataType, Field, TimeUnit, TimestampMicrosecondType,
    TimestampMillisecondType, TimestampNanosecondType, TimestampSecondType, UnionMode,
};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use egui::{RichText, Ui};
use re_log_types::TimestampFormat;
use re_ui::list_item::{CustomContent, LabelContent};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{UiExt as _, UiLayout};

use crate::arrow_node::ArrowNode;
use crate::list_item_ranges::list_item_ranges;

/// Arrow display options.
///
/// Max item limits will not affect the `list_item`-based ui, that will always show all items.
pub struct DisplayOptions<'a> {
    /// Format options for items formatted with arrows built-in formatter.
    pub format_options: FormatOptions<'a>,

    /// Format for timestamp values.
    pub timestamp_format: TimestampFormat,

    /// How many items should be shown for arrays that have nested items?
    pub max_nested_array_items: usize,

    /// How many items should be shown for arrays that do not have nested items?
    pub max_array_items: usize,

    /// How many items should be shown for maps?
    pub max_map_items: usize,

    /// How many items should be shown for structs?
    pub max_struct_items: usize,

    /// Each nested level, by how much should the number of shown items decrease?
    pub decrease_nested_items_per_nested_level: usize,
}

impl Default for DisplayOptions<'_> {
    fn default() -> Self {
        Self {
            format_options: FormatOptions::default()
                .with_null("null")
                .with_display_error(true),
            timestamp_format: Default::default(),
            max_nested_array_items: 3,
            max_array_items: 6,
            max_map_items: 3,
            max_struct_items: 6,
            decrease_nested_items_per_nested_level: 1,
        }
    }
}

impl DisplayOptions<'_> {
    fn nested(&self) -> Self {
        Self {
            format_options: self.format_options.clone(),
            timestamp_format: self.timestamp_format,
            max_nested_array_items: self
                .max_nested_array_items
                .saturating_sub(self.decrease_nested_items_per_nested_level),
            max_array_items: self
                .max_array_items
                .saturating_sub(self.decrease_nested_items_per_nested_level),
            max_map_items: self
                .max_map_items
                .saturating_sub(self.decrease_nested_items_per_nested_level),
            max_struct_items: self
                .max_struct_items
                .saturating_sub(self.decrease_nested_items_per_nested_level),
            decrease_nested_items_per_nested_level: self.decrease_nested_items_per_nested_level,
        }
    }

    fn max_array_items(&self, nested: bool) -> usize {
        if nested {
            self.max_nested_array_items
        } else {
            self.max_array_items
        }
    }
}

pub struct ArrayUi<'a> {
    array: &'a dyn Array,
    show_index: Box<dyn ShowIndex + 'a>,
    max_items: usize,
}

impl<'a> ArrayUi<'a> {
    /// Returns an [`ArrayUi`] that can be used to show `array`
    ///
    /// This returns an error if an array of the given data type cannot be formatted/shown.
    pub fn try_new(array: &'a dyn Array, options: &DisplayOptions<'a>) -> Result<Self, ArrowError> {
        let show = make_ui(array, options)?;
        Ok(Self {
            array,
            max_items: options.max_array_items(show.is_item_nested()),
            show_index: show,
        })
    }

    /// Show a single value at `idx`.
    ///
    /// This will create a list item that might have some nested children.
    /// The list item will _not_ display the index.
    pub fn show_value(&self, idx: usize, ui: &mut Ui) {
        self.show_index.show(idx, ui);
    }

    /// Show a `list_item` based tree view of the data.
    pub fn show(&self, ui: &mut Ui) {
        list_ui(ui, 0..self.array.len(), &*self.show_index);
    }

    /// Returns a [`SyntaxHighlightedBuilder`] that displays the entire array.
    pub fn highlighted(&self) -> Result<SyntaxHighlightedBuilder, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new();
        write_list(
            &mut highlighted,
            0..self.array.len(),
            self.max_items,
            &*self.show_index,
        )?;
        Ok(highlighted)
    }

    /// Returns a [`SyntaxHighlightedBuilder`] that displays a single value at `idx`.
    pub fn value_highlighted(&self, idx: usize) -> Result<SyntaxHighlightedBuilder, ArrowError> {
        let mut highlighted = SyntaxHighlightedBuilder::new();
        self.show_index.write(idx, &mut highlighted)?;
        Ok(highlighted)
    }
}

fn make_ui<'a>(
    array: &'a dyn Array,
    options: &DisplayOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    downcast_integer_array! {
        array => show_custom(array, options),
        DataType::Float16 => show_custom(array.as_primitive::<Float16Type>(), options),
        DataType::Float32 => show_custom(array.as_primitive::<Float32Type>(), options),
        DataType::Float64 => show_custom(array.as_primitive::<Float64Type>(), options),
        DataType::Null | DataType::Boolean | DataType::Utf8 | DataType::LargeUtf8
        | DataType::Utf8View | DataType::BinaryView
        | DataType::Date32 | DataType::Date64 | DataType::Time32(_) | DataType::Time64(_)
        | DataType::Duration(_) | DataType::Interval(_)
        | DataType::Decimal32(_, _) | DataType::Decimal64(_, _) | DataType::Decimal128(_, _) | DataType::Decimal256(_, _)
        => {
            show_arrow_builtin(array, options)
        }
        DataType::Timestamp(TimeUnit::Second, _) => {
            show_custom(array.as_primitive::<TimestampSecondType>(), options)
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            show_custom(array.as_primitive::<TimestampMillisecondType>(), options)
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
           show_custom(array.as_primitive::<TimestampMicrosecondType>(), options)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            show_custom(array.as_primitive::<TimestampNanosecondType>(), options)
        }
        DataType::FixedSizeBinary(_) => {
            let a = array
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .expect("FixedSizeBinaryArray downcast failed");
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
            let a = array
                .as_any()
                .downcast_ref::<FixedSizeListArray>()
                .expect("FixedSizeListArray downcast failed");
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
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
        let mut text = String::new();
        self.formatter.value(idx).write(&mut text)?;

        let dt = self.array.data_type();

        if self.array.is_null(idx) {
            f.append_null("null");
        } else if matches!(
            dt,
            DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View
        ) {
            f.append_string_value(&text);
        } else {
            f.append_primitive(&text);
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
    options: &DisplayOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    Ok(Box::new(ShowBuiltIn {
        formatter: ArrayFormatter::try_new(array, &options.format_options)?,
        array,
    }))
}

type EmptyArrowResult = Result<(), ArrowError>;

/// [`egui::Widget`] for arrays.
///
/// This is implemented for all the different arrow array types.
///
/// In addition to displaying the value as a rerun `list_item` it can also be formatted with syntax
/// highlighting via [`Self::write`].
///
/// UI-equivalent of arrows `DisplayIndex` trait.
pub(crate) trait ShowIndex {
    /// Append the item at `idx` to the given [`SyntaxHighlightedBuilder`].
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult;

    /// Show the item at `idx` as a rerun `list_item`.
    fn show(&self, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new();
        let result = self.write(idx, &mut highlighted);
        match result {
            Ok(()) => {
                ui.list_item().show_hierarchical(
                    ui,
                    CustomContent::new(|ui, _context| {
                        UiLayout::List.data_label(ui, highlighted);
                    }),
                );
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

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError>;

    fn write(
        &self,
        state: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult;

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let mut highlighted = SyntaxHighlightedBuilder::new();
        let result = self.write(state, idx, &mut highlighted);
        match result {
            Ok(()) => {
                ui.list_item().show_hierarchical(
                    ui,
                    LabelContent::new(highlighted.into_widget_text(ui.style())),
                );
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

    fn prepare(&self, _options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
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
    options: &DisplayOptions<'a>,
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
                f.append_null(self.null);
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

macro_rules! numeric_primitive_display {
    ($fmt:path: $($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
                let value = self.value(idx);
                let s = $fmt(value);
                f.append_primitive(&s);
                Ok(())
            }

            fn array(&self) -> &dyn Array {
                self
            }
        })+
    };
}

numeric_primitive_display!(re_format::format_int: Int8Type, Int16Type, Int32Type, Int64Type);
numeric_primitive_display!(re_format::format_uint: UInt8Type, UInt16Type, UInt32Type, UInt64Type);
numeric_primitive_display!(re_format::format_f32: Float32Type);
numeric_primitive_display!(re_format::format_f64: Float64Type);
numeric_primitive_display!(re_format::format_f16: Float16Type);

macro_rules! timestamp_primitive_display {
    ($t:ty, $conv_fn:ident ) => {
        impl<'a> ShowIndexState<'a> for &'a $t {
            type State = TimestampFormat;

            fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
                Ok(options.timestamp_format)
            }

            fn write(
                &self,
                state: &Self::State,
                idx: usize,
                f: &mut SyntaxHighlightedBuilder,
            ) -> EmptyArrowResult {
                if self.is_null(idx) {
                    f.append_primitive("null");
                } else {
                    #[allow(clippy::allow_attributes, trivial_numeric_casts)]
                    let timestamp = jiff::Timestamp::$conv_fn(self.value(idx) as _)
                        .map_err(|err| ArrowError::ExternalError(Box::new(err)))?;
                    f.append_primitive(&re_log_types::Timestamp::from(timestamp).format(*state));
                }

                Ok(())
            }
        }
    };
}

timestamp_primitive_display!(TimestampSecondArray, from_second);
timestamp_primitive_display!(TimestampMillisecondArray, from_millisecond);
timestamp_primitive_display!(TimestampMicrosecondArray, from_microsecond);
timestamp_primitive_display!(TimestampNanosecondArray, from_nanosecond);

impl<OffsetSize: OffsetSizeTrait> ShowIndex for &GenericBinaryArray<OffsetSize> {
    fn write(&self, idx: usize, f: &mut SyntaxHighlightedBuilder) -> EmptyArrowResult {
        let value = self.value(idx);
        f.append_primitive(&re_format::format_bytes(value.len() as f64));
        Ok(())
    }

    fn array(&self) -> &dyn Array {
        self
    }
}

impl<'a, K: ArrowDictionaryKeyType> ShowIndexState<'a> for &'a DictionaryArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
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

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
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
    max_items: usize,
    values: &dyn ShowIndex,
) -> EmptyArrowResult {
    f.append_syntax("[");
    if max_items == 0 && !range.is_empty() {
        f.append_syntax("…");
    } else {
        if let Some(idx) = range.next() {
            values.write(idx, f)?;
        }

        let mut items = 1;

        for idx in range {
            if items >= max_items {
                f.append_syntax(", …");
                break;
            }
            f.append_syntax(", ");
            values.write(idx, f)?;
            items += 1;
        }
    }
    f.append_syntax("]");
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
    type State = (Box<dyn ShowIndex + 'a>, usize);

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
        let show = make_ui(self.values().as_ref(), &options.nested())?;
        let max_items = options.max_array_items(show.is_item_nested());
        Ok((show, max_items))
    }

    fn write(
        &self,
        (show_index, max_items): &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        write_list(f, start..end, *max_items, show_index.as_ref())
    }

    fn show(&self, (show_index, _): &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        list_ui(ui, start..end, show_index.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
    }
}

struct FixedSizeListArrayState<'a> {
    value_length: usize,
    values: Box<dyn ShowIndex + 'a>,
    max_items: usize,
}

impl<'a> ShowIndexState<'a> for &'a FixedSizeListArray {
    type State = FixedSizeListArrayState<'a>;

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
        let values = make_ui(self.values().as_ref(), &options.nested())?;
        let length = self.value_length();
        Ok(FixedSizeListArrayState {
            value_length: length as usize,
            max_items: options.max_array_items(values.is_item_nested()),
            values,
        })
    }

    fn write(
        &self,
        FixedSizeListArrayState {
            value_length,
            values,
            max_items,
        }: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let start = idx * *value_length;
        let end = start + *value_length;
        write_list(f, start..end, *max_items, values.as_ref())
    }

    fn show(
        &self,
        FixedSizeListArrayState {
            value_length,
            values,
            max_items: _,
        }: &Self::State,
        idx: usize,
        ui: &mut Ui,
    ) {
        let start = idx * *value_length;
        let end = start + *value_length;
        list_ui(ui, start..end, values.as_ref());
    }

    fn is_item_nested(&self) -> bool {
        true
    }
}

/// Pairs a boxed [`ShowIndex`] with its field name
type FieldDisplay<'a> = (&'a Field, Box<dyn ShowIndex + 'a>);

struct FieldDisplayState<'a> {
    items: Vec<FieldDisplay<'a>>,
    max_items: usize,
}

impl<'a> ShowIndexState<'a> for &'a StructArray {
    type State = FieldDisplayState<'a>;

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
        let fields = self.fields();
        let nested_options = options.nested();

        let items = self
            .columns()
            .iter()
            .zip(fields)
            .map(|(a, f)| {
                let format = make_ui(a.as_ref(), &nested_options)?;
                Ok((&**f, format))
            })
            .collect::<Result<_, ArrowError>>()?;
        Ok(FieldDisplayState {
            items,
            max_items: options.max_struct_items,
        })
    }

    fn write(
        &self,
        FieldDisplayState { items, max_items }: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let mut iter = items.iter();
        f.append_syntax("{");
        if *max_items == 0 && !items.is_empty() {
            f.append_syntax("…");
        } else {
            if let Some((field, display)) = iter.next() {
                f.append_identifier(field.name());
                f.append_syntax(": ");
                display.as_ref().write(idx, f)?;
            }
            let mut items = 1;
            for (field, display) in iter {
                if items >= *max_items {
                    f.append_syntax(", …");
                    break;
                }
                f.append_syntax(", ");
                f.append_identifier(field.name());
                f.append_syntax(": ");
                display.as_ref().write(idx, f)?;
                items += 1;
            }
        }
        f.append_syntax("}");
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        for (field, show_field) in &state.items {
            let node = ArrowNode::field(field, show_field.as_ref());
            node.show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        self.data_type().is_nested()
    }
}

struct MapArrayState<'a> {
    keys: Box<dyn ShowIndex + 'a>,
    values: Box<dyn ShowIndex + 'a>,
    max_items: usize,
}

impl<'a> ShowIndexState<'a> for &'a MapArray {
    type State = MapArrayState<'a>;

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
        let nested = options.nested();
        let keys = make_ui(self.keys().as_ref(), &nested)?;
        let values = make_ui(self.values().as_ref(), &nested)?;
        Ok(MapArrayState {
            keys,
            values,
            max_items: options.max_map_items,
        })
    }

    fn write(
        &self,
        MapArrayState {
            keys,
            values,
            max_items,
        }: &Self::State,
        idx: usize,
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        f.append_syntax("{");
        if *max_items == 0 && !iter.is_empty() {
            f.append_syntax("…");
        } else {
            if let Some(idx) = iter.next() {
                keys.write(idx, f)?;
                f.append_syntax(": ");
                values.write(idx, f)?;
            }

            let mut items = 1;

            for idx in iter {
                if items >= *max_items {
                    f.append_syntax(", …");
                    break;
                }
                f.append_syntax(", ");
                keys.write(idx, f)?;
                f.append_syntax(": ");
                values.write(idx, f)?;
                items += 1;
            }
        }
        f.append_syntax("}");
        Ok(())
    }

    fn show(
        &self,
        MapArrayState {
            keys,
            values,
            max_items: _,
        }: &Self::State,
        idx: usize,
        ui: &mut Ui,
    ) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let iter = start..end;

        for idx in iter {
            let mut key_string = SyntaxHighlightedBuilder::new();
            let result = keys.write(idx, &mut key_string);
            let text = if result.is_err() {
                RichText::new("cannot display key")
                    .color(ui.tokens().error_fg_color)
                    .into()
            } else {
                key_string.into_widget_text(ui.style())
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

    fn prepare(&self, options: &DisplayOptions<'a>) -> Result<Self::State, ArrowError> {
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
        f: &mut SyntaxHighlightedBuilder,
    ) -> EmptyArrowResult {
        let id = self.type_id(idx);
        let idx = match mode {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (field, show_field) = fields[id as usize]
            .as_ref()
            .expect("Union field should be present");

        f.append_syntax("{");
        f.append_identifier(field.name());
        f.append_syntax("=");
        show_field.write(idx, f)?;
        f.append_syntax("}");

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
