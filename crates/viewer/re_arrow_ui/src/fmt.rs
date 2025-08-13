//! [`ArrowUi`] can be used to show arbitrary Arrow data with a nice UI.
//! The implementation is inspired from arrows built-in display formatter:
//! https://github.com/apache/arrow-rs/blob/c628435f9f14abc645fb546442132974d3d380ca/arrow-cast/src/display.rs
use arrow::array::cast::*;
use arrow::array::types::*;
use arrow::array::*;
use arrow::datatypes::{ArrowNativeType, DataType, UnionMode};
use arrow::error::ArrowError;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use arrow::*;
use egui::Ui;
use half::f16;
use re_format::{format_f32, format_f64, format_int, format_uint};
use re_ui::UiExt;
use re_ui::list_item::PropertyContent;
use std::fmt::{Display, Formatter, Write};
use std::ops::Range;

pub struct ArrayUi<'a> {
    format: Box<dyn ShowIndex + 'a>,
}

impl<'a> ArrayUi<'a> {
    /// Returns an [`ArrayUi`] that can be used to format `array`
    ///
    /// This returns an error if an array of the given data type cannot be formatted
    pub fn try_new(array: &'a dyn Array, options: &FormatOptions<'a>) -> Result<Self, ArrowError> {
        Ok(Self {
            format: make_ui(array, options)?,
        })
    }

    pub fn show_value(&self, idx: usize, ui: &mut Ui) {
        // self.format.show(idx, ui);
        let node = ArrowNode::index(idx, &*self.format);
        node.show(ui, idx);
    }
}

fn make_ui<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    downcast_primitive_array! {
        array => {
            downcast_integer_array! {
                // We have a custom implementation for integer
                array => show_custom(array, options),
                _ => show_arrow_builtin(array, options),
            }
        },
        DataType::Float16 => show_custom(array.as_primitive::<Float16Type>(), options),
        DataType::Float32 => show_custom(array.as_primitive::<Float32Type>(), options),
        DataType::Float64 => show_custom(array.as_primitive::<Float64Type>(), options),
        // Should we have custom display impl for these?
        // DataType::Decimal128(_, _) => show_custom(array.as_primitive::<Decimal128Type>(), options),
        // DataType::Decimal256(_, _) => show_custom(array.as_primitive::<Decimal256Type>(), options),
        DataType::Null => show_arrow_builtin(as_null_array(array), options),
        DataType::Boolean => show_arrow_builtin(as_boolean_array(array), options),
        DataType::Utf8 => show_arrow_builtin(array.as_string::<i32>(), options),
        DataType::LargeUtf8 => show_arrow_builtin(array.as_string::<i64>(), options),
        DataType::Utf8View => show_arrow_builtin(array.as_string_view(), options),
        DataType::Binary => show_arrow_builtin(array.as_binary::<i32>(), options),
        DataType::BinaryView => show_arrow_builtin(array.as_binary_view(), options),
        DataType::LargeBinary => show_arrow_builtin(array.as_binary::<i64>(), options),
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
        d => Err(ArrowError::NotYetImplemented(format!("formatting {d} is not yet supported"))),
    }
}

impl<'a> ShowIndex for ArrayFormatter<'a> {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        self.value(idx).write(f)?;
        Ok(())
    }
}

fn show_arrow_builtin<'a>(
    array: &'a dyn Array,
    options: &FormatOptions<'a>,
) -> Result<Box<dyn ShowIndex + 'a>, ArrowError> {
    Ok(Box::new(ArrayFormatter::try_new(array, options)?))
}

struct ShowPrimitive<'a> {
    formatter: arrow::util::display::ArrayFormatter<'a>,
    array: &'a dyn Array,
}

impl<'a> ShowIndexState<'a> for ShowPrimitive<'a> {
    type State = arrow::util::display::ArrayFormatter<'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        let formatter = arrow::util::display::ArrayFormatter::try_new(self.array, options)?;
        Ok(formatter)
    }

    fn write(&self, state: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        state.value(idx).write(f)?;
        Ok(())
    }
}

/// Either an [`ArrowError`] or [`std::fmt::Error`]
enum FormatError {
    Format(std::fmt::Error),
    Arrow(ArrowError),
}

type FormatResult = Result<(), FormatError>;

impl From<std::fmt::Error> for FormatError {
    fn from(value: std::fmt::Error) -> Self {
        Self::Format(value)
    }
}

impl From<ArrowError> for FormatError {
    fn from(value: ArrowError) -> Self {
        Self::Arrow(value)
    }
}

/// [`Display`] but accepting an index
trait ShowIndex {
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult;

    fn show(&self, idx: usize, ui: &mut Ui) {
        let mut text = String::new();
        let result = self.write(idx, &mut text);
        ui.label(text);
    }

    fn is_item_nested(&self) -> bool {
        false
    }
}

/// [`ShowIndex`] with additional state
trait ShowIndexState<'a> {
    type State;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError>;

    fn write(&self, state: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult;

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let mut text = String::new();
        let result = self.write(state, idx, &mut text);
        ui.label(text);
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

    fn write(&self, _: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
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
    fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
        if self.array.is_null(idx) {
            if !self.null.is_empty() {
                f.write_str(self.null)?
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
}

macro_rules! primitive_display {
    ($fmt:ident: $($t:ty),+) => {
        $(impl<'a> ShowIndex for &'a PrimitiveArray<$t>
        {
            fn write(&self, idx: usize, f: &mut dyn Write) -> FormatResult {
                let value = self.value(idx);
                let s = $fmt(value);
                f.write_str(&s)?;
                Ok(())
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

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.keys().values()[idx].as_usize();
        s.as_ref().write(value_idx, f)
    }
}

impl<'a, K: RunEndIndexType> ShowIndexState<'a> for &'a RunArray<K> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let value_idx = self.get_physical_index(idx);
        s.as_ref().write(value_idx, f)
    }
}

fn write_list(f: &mut dyn Write, mut range: Range<usize>, values: &dyn ShowIndex) -> FormatResult {
    f.write_char('[')?;
    if let Some(idx) = range.next() {
        values.write(idx, f)?;
    }
    for idx in range {
        write!(f, ", ")?;
        values.write(idx, f)?;
    }
    f.write_char(']')?;
    Ok(())
}

enum NodeLabel {
    Index(usize),
    Name(String),
}
pub struct ArrowNode<'a> {
    label: NodeLabel,
    values: &'a dyn ShowIndex,
}

impl<'a> ArrowNode<'a> {
    pub fn name(name: &str, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Name(name.to_string()),
            values,
        }
    }

    pub fn index(idx: usize, values: &'a dyn ShowIndex) -> Self {
        Self {
            label: NodeLabel::Index(idx),
            values,
        }
    }

    pub fn show(&self, ui: &mut Ui, index: usize) {
        let label = match &self.label {
            NodeLabel::Index(idx) => format!("{idx}"),
            NodeLabel::Name(name) => name.clone(),
        };

        let mut value = String::new();
        self.values.write(index, &mut value); // TODO: Handle error

        let nested = self.values.is_item_nested();

        let mut item = ui.list_item();
        let id = ui.unique_id().with(index).with(&label);
        let mut content = PropertyContent::new(label).value_text(value);

        if nested {
            item.show_hierarchical_with_children(ui, id, false, content, |ui| {
                self.values.show(index, ui);
            });
        } else {
            item.show_hierarchical(ui, content);
        }
    }
}

fn list_ui(ui: &mut Ui, mut range: Range<usize>, values: &dyn ShowIndex) {
    let mut label_index = 0;
    for idx in range {
        let node = ArrowNode::index(label_index, values);
        node.show(ui, idx);
        label_index += 1;
    }
}

impl<'a, O: OffsetSizeTrait> ShowIndexState<'a> for &'a GenericListArray<O> {
    type State = Box<dyn ShowIndex + 'a>;

    fn prepare(&self, options: &FormatOptions<'a>) -> Result<Self::State, ArrowError> {
        make_ui(self.values().as_ref(), options)
    }

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
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
        dbg!(self.data_type());
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

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
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

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let mut iter = s.iter();
        f.write_char('{')?;
        if let Some((name, display)) = iter.next() {
            write!(f, "{name}: ")?;
            display.as_ref().write(idx, f)?;
        }
        for (name, display) in iter {
            write!(f, ", {name}: ")?;
            display.as_ref().write(idx, f)?;
        }
        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        for (name, display) in state {
            let node = ArrowNode::name(name, display.as_ref());
            node.show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
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

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        f.write_char('{')?;
        if let Some(idx) = iter.next() {
            s.0.write(idx, f)?;
            write!(f, ": ")?;
            s.1.write(idx, f)?;
        }

        for idx in iter {
            write!(f, ", ")?;
            s.0.write(idx, f)?;
            write!(f, ": ")?;
            s.1.write(idx, f)?;
        }

        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let offsets = self.value_offsets();
        let end = offsets[idx + 1].as_usize();
        let start = offsets[idx].as_usize();
        let mut iter = start..end;

        for idx in iter {
            let mut key_string = String::new();
            state.0.write(idx, &mut key_string);

            ArrowNode::name(&key_string, state.1.as_ref()).show(ui, idx);
        }
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
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

    fn write(&self, s: &Self::State, idx: usize, f: &mut dyn Write) -> FormatResult {
        let id = self.type_id(idx);
        let idx = match s.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = s.0[id as usize].as_ref().unwrap();

        write!(f, "{{{name}=")?;
        field.write(idx, f)?;
        f.write_char('}')?;
        Ok(())
    }

    fn show(&self, state: &Self::State, idx: usize, ui: &mut Ui) {
        let id = self.type_id(idx);
        let idx = match state.1 {
            UnionMode::Dense => self.value_offset(idx),
            UnionMode::Sparse => idx,
        };
        let (name, field) = state.0[id as usize].as_ref().unwrap();

        let node = ArrowNode::name(name, field.as_ref());
        node.show(ui, idx);
    }

    fn is_item_nested(&self) -> bool {
        let data_type = self.data_type();
        dbg!(data_type);
        data_type.is_nested()
    }
}
