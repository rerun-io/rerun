//! Decodes CDR-encoded ROS 2 messages straight into Arrow builders.
//!
//! Based on a [`MessageDecodePlan`], Arrow datatypes and builders are derived for a message type,
//! and [`CdrArrowDecoder`] provides the direct CDR -> Arrow decoding path for it.

use std::sync::Arc;

use anyhow::Context as _;
use arrow::array::{
    Array as _, ArrayBuilder, ArrowPrimitiveType, BooleanArray, BooleanBuilder, FixedSizeListArray,
    FixedSizeListBuilder, Float32Builder, Float64Builder, Int8Builder, Int16Builder, Int32Builder,
    Int64Builder, ListBuilder, PrimitiveBuilder, StringBuilder, StructBuilder, UInt8Builder,
    UInt16Builder, UInt32Builder, UInt64Builder,
};
use arrow::datatypes::{
    DataType, Field, Fields, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type,
    UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use re_ros_msg::message_spec::{ArraySize, BuiltInType};

use crate::parsers::dds;

use super::Ros2ReflectionError;
use super::decode_plan::{FieldLayout, MessageDecodePlan, ValueLayout};

/// CDR has no notion of an unset field: every field of every message is always present on the
/// wire, so a successfully decoded row is never null.
///
/// A row whose decode failed does get nulled out, but [`append_null_for_message`] nulls the
/// containing struct along with its children, and `StructArray` permits child nulls that the
/// struct's own null masks. [`CdrArrowDecoder::finish`] then drops the row altogether, so no null
/// survives into the returned array.
const FIELD_NULLABLE: bool = false;

/// Why [`CdrArrowDecoder::decode_message`] rejected a message.
#[derive(Debug)]
pub(super) enum CdrDecodeError {
    /// The message was rejected. Its row is cancelled, so decoding can continue.
    Message(anyhow::Error),

    /// The Arrow builders could not be returned to a row boundary, so the decoder is unusable.
    Unrecoverable(Ros2ReflectionError),
}

/// Decodes CDR messages into one Arrow column, owning the builders its plan maps to.
pub(super) struct CdrArrowDecoder {
    plan: Arc<MessageDecodePlan>,

    /// One row per message, each holding a single decoded message struct.
    builder: FixedSizeListBuilder<MessageStructBuilder>,

    /// Rows cancelled by [`Self::cancel_row`], dropped again by [`Self::finish`].
    ///
    /// Empty unless the channel contains a corrupt message.
    cancelled_rows: Vec<usize>,
}

impl CdrArrowDecoder {
    /// Creates the Arrow builders that `plan` maps to, with room for `num_rows` messages.
    pub(super) fn new(plan: Arc<MessageDecodePlan>, num_rows: usize) -> Self {
        let struct_builder = struct_builder_for_message(&plan, MessageDecodePlan::ROOT_ID);

        Self {
            plan,
            builder: FixedSizeListBuilder::with_capacity(struct_builder, 1, num_rows),
            cancelled_rows: Vec::new(),
        }
    }

    /// The plan these builders were derived from.
    pub(super) fn plan(&self) -> &MessageDecodePlan {
        &self.plan
    }

    /// Decodes one CDR message, appending a row.
    ///
    /// A message that fails to decode has its row cancelled, so decoding can carry on with the
    /// next one.
    pub(super) fn decode_message(&mut self, buf: &[u8]) -> Result<(), CdrDecodeError> {
        if let Err(source) =
            decode_bytes_into_arrow(&self.plan, buf, &mut self.builder.values().builder)
        {
            self.cancel_row().map_err(CdrDecodeError::Unrecoverable)?;
            return Err(CdrDecodeError::Message(source));
        }
        self.builder.append(true);

        Ok(())
    }

    /// Cancels the row that a failed decode left part-way written.
    ///
    /// Decoding writes into the Arrow builders as it walks the CDR stream, so a failure can leave
    /// the field builders at unequal lengths — which [`StructBuilder::finish`] rejects outright.
    /// This brings them back to a row boundary and marks the row for removal.
    fn cancel_row(&mut self) -> Result<(), Ros2ReflectionError> {
        append_null_for_message(
            &self.plan,
            MessageDecodePlan::ROOT_ID,
            &mut self.builder.values().builder,
        )?;

        self.cancelled_rows.push(self.builder.len());
        self.builder.append(false);

        Ok(())
    }

    /// The decoded messages, with any cancelled rows removed.
    pub(super) fn finish(&mut self) -> FixedSizeListArray {
        let messages = self.builder.finish();
        if self.cancelled_rows.is_empty() {
            return messages;
        }

        let mut keep = vec![true; messages.len()];
        for &row in &self.cancelled_rows {
            keep[row] = false;
        }

        re_arrow_util::filter_array(&messages, &BooleanArray::from(keep))
    }
}

/// Minimal wrapper around [`StructBuilder`] for use as a nested Arrow builder.
struct MessageStructBuilder {
    builder: StructBuilder,
}

impl ArrayBuilder for MessageStructBuilder {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn into_box_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    fn len(&self) -> usize {
        self.builder.len()
    }

    fn is_empty(&self) -> bool {
        self.builder.is_empty()
    }

    fn finish(&mut self) -> arrow::array::ArrayRef {
        Arc::new(self.builder.finish())
    }

    fn finish_cloned(&self) -> arrow::array::ArrayRef {
        Arc::new(self.builder.finish_cloned())
    }
}

/// The Arrow field for one message field. Shared so that the declared datatype in
/// [`datatype_from_layout`] and the builder in [`struct_builder_for_message`] cannot drift apart.
fn arrow_field_from_layout(plan: &MessageDecodePlan, field: &FieldLayout) -> Field {
    Field::new(
        field.name(),
        datatype_from_layout(plan, field.value()),
        FIELD_NULLABLE,
    )
}

/// Whether the elements of a CDR array or sequence can be null.
///
/// CDR itself has no null elements. But a decode that fails part-way through an array of messages
/// closes the half-written element with a null (see [`append_null_for_layout`]) before nulling the
/// array row, and [`CdrArrowDecoder::finish`] only drops that row once the array is already built.
/// Unlike `StructArray`, `ListArray` rejects a null element under a non-nullable field even when
/// the row holding it is itself null — so arrays of messages have to stay nullable.
///
/// Scalar builders are always at a row boundary, so arrays of scalars never gain such an element.
fn array_item_nullable(element: &ValueLayout) -> bool {
    matches!(element, ValueLayout::Message(_))
}

/// Creates Arrow builders whose recursive structure mirrors `message_id` in `plan`.
fn struct_builder_for_message(plan: &MessageDecodePlan, message_id: usize) -> MessageStructBuilder {
    let (fields, field_builders): (Vec<Field>, Vec<Box<dyn ArrayBuilder>>) = plan
        .message(message_id)
        .fields()
        .iter()
        .map(|field| {
            (
                arrow_field_from_layout(plan, field),
                arrow_builder_from_layout(plan, field.value()),
            )
        })
        .unzip();

    MessageStructBuilder {
        builder: StructBuilder::new(fields, field_builders),
    }
}

/// Creates an Arrow builder for one value layout.
fn arrow_builder_from_layout(
    plan: &MessageDecodePlan,
    value_layout: &ValueLayout,
) -> Box<dyn ArrayBuilder> {
    match value_layout {
        ValueLayout::BuiltIn(ty) => arrow_builder_from_builtin_type(ty),
        ValueLayout::Message(message_id) => Box::new(struct_builder_for_message(plan, *message_id)),
        ValueLayout::Array { element, .. } => {
            let item_field = Field::new_list_field(
                datatype_from_layout(plan, element),
                array_item_nullable(element),
            );
            // `ListBuilder` defaults to a nullable item field, so spell ours out to keep the built
            // array's datatype equal to the one `datatype_from_layout` declares.
            Box::new(
                ListBuilder::new(arrow_builder_from_layout(plan, element)).with_field(item_field),
            )
        }
    }
}

/// Creates the Arrow datatype for one value layout.
fn datatype_from_layout(plan: &MessageDecodePlan, value_layout: &ValueLayout) -> DataType {
    match value_layout {
        ValueLayout::BuiltIn(ty) => datatype_from_builtin_type(ty),
        ValueLayout::Message(message_id) => DataType::Struct(
            plan.message(*message_id)
                .fields()
                .iter()
                .map(|field| arrow_field_from_layout(plan, field))
                .collect::<Fields>(),
        ),
        ValueLayout::Array { element, .. } => DataType::new_list(
            datatype_from_layout(plan, element),
            array_item_nullable(element),
        ),
    }
}

/// Creates the Arrow builder for one ROS built-in type.
fn arrow_builder_from_builtin_type(ty: &BuiltInType) -> Box<dyn ArrayBuilder> {
    match ty {
        BuiltInType::Bool => Box::new(BooleanBuilder::new()),
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => Box::new(UInt8Builder::new()),
        BuiltInType::Int8 => Box::new(Int8Builder::new()),
        BuiltInType::Int16 => Box::new(Int16Builder::new()),
        BuiltInType::UInt16 => Box::new(UInt16Builder::new()),
        BuiltInType::Int32 => Box::new(Int32Builder::new()),
        BuiltInType::UInt32 => Box::new(UInt32Builder::new()),
        BuiltInType::Int64 => Box::new(Int64Builder::new()),
        BuiltInType::UInt64 => Box::new(UInt64Builder::new()),
        BuiltInType::Float32 => Box::new(Float32Builder::new()),
        BuiltInType::Float64 => Box::new(Float64Builder::new()),
        BuiltInType::String(_) | BuiltInType::WString(_) => Box::new(StringBuilder::new()),
    }
}

/// Creates the Arrow datatype for one ROS built-in type.
fn datatype_from_builtin_type(ty: &BuiltInType) -> DataType {
    match ty {
        BuiltInType::Bool => DataType::Boolean,
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => DataType::UInt8,
        BuiltInType::Int8 => DataType::Int8,
        BuiltInType::Int16 => DataType::Int16,
        BuiltInType::UInt16 => DataType::UInt16,
        BuiltInType::Int32 => DataType::Int32,
        BuiltInType::UInt32 => DataType::UInt32,
        BuiltInType::Int64 => DataType::Int64,
        BuiltInType::UInt64 => DataType::UInt64,
        BuiltInType::Float32 => DataType::Float32,
        BuiltInType::Float64 => DataType::Float64,
        BuiltInType::String(_) | BuiltInType::WString(_) => DataType::Utf8, // No wstring in Arrow
    }
}

/// Decodes a ROS 2 CDR message directly into its Arrow builders.
fn decode_bytes_into_arrow(
    plan: &MessageDecodePlan,
    buf: &[u8],
    builder: &mut StructBuilder,
) -> anyhow::Result<()> {
    // Note: only the 4-byte encapsulation header is checked here. The body is validated later as the fields are read.
    if buf.len() < 4 {
        anyhow::bail!(
            "message is too short to hold a CDR encapsulation header: {} bytes, expected at least 4",
            buf.len()
        );
    }

    let representation_identifier = dds::RepresentationIdentifier::from_bytes([buf[0], buf[1]])
        .with_context(|| "failed to parse CDR representation identifier")?;
    anyhow::ensure!(
        representation_identifier.is_cdr() || representation_identifier.is_cdr2(),
        "message is not encoded using a CDR representation: {representation_identifier:?}"
    );

    if representation_identifier.is_big_endian() {
        let mut reader = re_cdr::CdrReader::<byteorder::BigEndian>::new(&buf[4..]);
        decode_message_into_arrow(plan, &mut reader, MessageDecodePlan::ROOT_ID, builder)
    } else {
        let mut reader = re_cdr::CdrReader::<byteorder::LittleEndian>::new(&buf[4..]);
        decode_message_into_arrow(plan, &mut reader, MessageDecodePlan::ROOT_ID, builder)
    }
    .with_context(|| "failed to deserialize CDR message")
}

/// Decodes one complete message into a structurally matching Arrow struct builder.
///
/// Both the CDR stream and the builder fields are consumed in ROS declaration order.
fn decode_message_into_arrow<BO: re_cdr::CdrEndian>(
    plan: &MessageDecodePlan,
    reader: &mut re_cdr::CdrReader<'_, BO>,
    message_id: usize,
    builder: &mut StructBuilder,
) -> anyhow::Result<()> {
    let message_layout = plan.message(message_id);
    re_log::debug_assert_eq!(
        message_layout.fields().len(),
        builder.field_builders().len(),
        "plan and Arrow builders must have the same number of fields"
    );

    for (field, field_builder) in
        std::iter::zip(message_layout.fields(), builder.field_builders_mut())
    {
        decode_value_into_arrow(plan, reader, field.value(), field_builder.as_mut())?;
    }
    builder.append(true);
    Ok(())
}

/// Decodes one value layout and appends exactly one value to its matching Arrow builder.
fn decode_value_into_arrow<BO: re_cdr::CdrEndian>(
    plan: &MessageDecodePlan,
    reader: &mut re_cdr::CdrReader<'_, BO>,
    value_layout: &ValueLayout,
    builder: &mut dyn ArrayBuilder,
) -> anyhow::Result<()> {
    match value_layout {
        ValueLayout::BuiltIn(ty) => decode_builtin_into_arrow(reader, ty, builder),
        ValueLayout::Message(message_id) => {
            let message_struct_builder = downcast_builder::<MessageStructBuilder>(builder)?;
            decode_message_into_arrow(
                plan,
                reader,
                *message_id,
                &mut message_struct_builder.builder,
            )
        }
        ValueLayout::Array { element, size } => {
            let count = match size {
                ArraySize::Fixed(len) => *len,
                ArraySize::Bounded(_) | ArraySize::Unbounded => reader.read_sequence_length()?,
            };
            if let ValueLayout::BuiltIn(ty) = element.as_ref() {
                return decode_builtin_array_into_arrow(reader, ty, builder, count);
            }
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            for _ in 0..count {
                decode_value_into_arrow(plan, reader, element, list_builder.values())?;
            }
            list_builder.append(true);
            Ok(())
        }
    }
}

/// Decodes one ROS built-in value and appends it to a matching scalar Arrow builder.
fn decode_builtin_into_arrow<BO: re_cdr::CdrEndian>(
    reader: &mut re_cdr::CdrReader<'_, BO>,
    ty: &BuiltInType,
    builder: &mut dyn ArrayBuilder,
) -> anyhow::Result<()> {
    match ty {
        BuiltInType::Bool => {
            downcast_builder::<BooleanBuilder>(builder)?.append_value(reader.read_bool()?);
        }
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => {
            downcast_builder::<UInt8Builder>(builder)?.append_value(reader.read_u8()?);
        }
        BuiltInType::Int8 => {
            downcast_builder::<Int8Builder>(builder)?.append_value(reader.read_i8()?);
        }
        BuiltInType::Int16 => {
            downcast_builder::<Int16Builder>(builder)?.append_value(reader.read_i16()?);
        }
        BuiltInType::UInt16 => {
            downcast_builder::<UInt16Builder>(builder)?.append_value(reader.read_u16()?);
        }
        BuiltInType::Int32 => {
            downcast_builder::<Int32Builder>(builder)?.append_value(reader.read_i32()?);
        }
        BuiltInType::UInt32 => {
            downcast_builder::<UInt32Builder>(builder)?.append_value(reader.read_u32()?);
        }
        BuiltInType::Int64 => {
            downcast_builder::<Int64Builder>(builder)?.append_value(reader.read_i64()?);
        }
        BuiltInType::UInt64 => {
            downcast_builder::<UInt64Builder>(builder)?.append_value(reader.read_u64()?);
        }
        BuiltInType::Float32 => {
            downcast_builder::<Float32Builder>(builder)?.append_value(reader.read_f32()?);
        }
        BuiltInType::Float64 => {
            downcast_builder::<Float64Builder>(builder)?.append_value(reader.read_f64()?);
        }
        BuiltInType::String(_) => {
            downcast_builder::<StringBuilder>(builder)?.append_value(reader.read_str()?);
        }
        BuiltInType::WString(_) => {
            anyhow::bail!("ROS 2 `wstring` decoding is not supported");
        }
    }
    Ok(())
}

/// Appends a bulk-read primitive array without per-element Arrow builder dispatch.
fn append_primitive_slice<T>(
    builder: &mut dyn ArrayBuilder,
    values: &[T::Native],
) -> Result<(), Ros2ReflectionError>
where
    T: ArrowPrimitiveType,
    PrimitiveBuilder<T>: 'static,
{
    let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
    let values_builder = downcast_builder::<PrimitiveBuilder<T>>(list_builder.values())?;
    values_builder.append_slice(values);
    list_builder.append(true);
    Ok(())
}

/// Decodes built-in arrays through the CDR reader's bulk numeric-read path where available.
fn decode_builtin_array_into_arrow<BO: re_cdr::CdrEndian>(
    reader: &mut re_cdr::CdrReader<'_, BO>,
    ty: &BuiltInType,
    builder: &mut dyn ArrayBuilder,
    count: usize,
) -> anyhow::Result<()> {
    match ty {
        BuiltInType::Bool => {
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_builder::<BooleanBuilder>(list_builder.values())?;
            for _ in 0..count {
                values_builder.append_value(reader.read_bool()?);
            }
            list_builder.append(true);
        }
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => {
            append_primitive_slice::<UInt8Type>(builder, reader.read_bytes(count)?)?;
        }
        BuiltInType::Int8 => {
            let values: Vec<i8> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Int8Type>(builder, &values)?;
        }
        BuiltInType::Int16 => {
            let values: Vec<i16> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Int16Type>(builder, &values)?;
        }
        BuiltInType::UInt16 => {
            let values: Vec<u16> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<UInt16Type>(builder, &values)?;
        }
        BuiltInType::Int32 => {
            let values: Vec<i32> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Int32Type>(builder, &values)?;
        }
        BuiltInType::UInt32 => {
            let values: Vec<u32> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<UInt32Type>(builder, &values)?;
        }
        BuiltInType::Int64 => {
            let values: Vec<i64> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Int64Type>(builder, &values)?;
        }
        BuiltInType::UInt64 => {
            let values: Vec<u64> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<UInt64Type>(builder, &values)?;
        }
        BuiltInType::Float32 => {
            let values: Vec<f32> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Float32Type>(builder, &values)?;
        }
        BuiltInType::Float64 => {
            let values: Vec<f64> = reader.read_numeric_vec(count)?;
            append_primitive_slice::<Float64Type>(builder, &values)?;
        }
        BuiltInType::String(_) => {
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            let values_builder = downcast_builder::<StringBuilder>(list_builder.values())?;
            for _ in 0..count {
                values_builder.append_value(reader.read_str()?);
            }
            list_builder.append(true);
        }
        BuiltInType::WString(_) => anyhow::bail!("ROS 2 `wstring` decoding is not supported"),
    }
    Ok(())
}

/// Downcasts a builder to the concrete type its layout calls for.
fn downcast_builder<T: std::any::Any>(
    builder: &mut dyn ArrayBuilder,
) -> Result<&mut T, Ros2ReflectionError> {
    builder.as_any_mut().downcast_mut::<T>().ok_or_else(|| {
        let type_name = std::any::type_name::<T>();
        Ros2ReflectionError::Downcast(type_name.strip_suffix("Builder").unwrap_or(type_name))
    })
}

/// Appends one null row for `message_id`, padding any field a failed decode left short.
///
/// Arrow requires a struct's children to stay as long as the struct itself, so even a null row has
/// to advance every field builder.
fn append_null_for_message(
    plan: &MessageDecodePlan,
    message_id: usize,
    builder: &mut StructBuilder,
) -> Result<(), Ros2ReflectionError> {
    let fields = plan.message(message_id).fields();
    let row_len = builder.len() + 1;

    for (index, field) in fields.iter().enumerate() {
        let field_builder = builder.field_builders_mut()[index].as_mut();
        if field_builder.len() < row_len {
            append_null_for_layout(plan, field.value(), field_builder)?;
        }
    }

    builder.append_null();

    Ok(())
}

/// Cancels the row of `builder` if a failed decode left one part-way written.
///
/// Such a row shows up as a field builder that has advanced further than the struct itself.
fn cancel_partial_struct_row(
    plan: &MessageDecodePlan,
    message_id: usize,
    builder: &mut StructBuilder,
) -> Result<(), Ros2ReflectionError> {
    let row_len = builder.len() + 1;
    let is_partial = builder
        .field_builders()
        .iter()
        .any(|field_builder| field_builder.len() >= row_len);

    if is_partial {
        append_null_for_message(plan, message_id, builder)?;
    }

    Ok(())
}

/// Appends one null value for `value_layout`, keeping nested builders at equal lengths.
fn append_null_for_layout(
    plan: &MessageDecodePlan,
    value_layout: &ValueLayout,
    builder: &mut dyn ArrayBuilder,
) -> Result<(), Ros2ReflectionError> {
    match value_layout {
        ValueLayout::BuiltIn(ty) => append_null_for_builtin(ty, builder)?,

        ValueLayout::Message(message_id) => {
            let message_struct_builder = downcast_builder::<MessageStructBuilder>(builder)?;
            append_null_for_message(plan, *message_id, &mut message_struct_builder.builder)?;
        }

        ValueLayout::Array { element, .. } => {
            let list_builder = downcast_builder::<ListBuilder<Box<dyn ArrayBuilder>>>(builder)?;
            // A failure inside an element leaves the values builder mid-row, so close that row
            // before closing the list row. ROS 2 has no nested arrays, so an element is either a
            // message or a scalar, and scalar builders are always at a row boundary.
            if let ValueLayout::Message(message_id) = element.as_ref() {
                let values = downcast_builder::<MessageStructBuilder>(list_builder.values())?;
                cancel_partial_struct_row(plan, *message_id, &mut values.builder)?;
            }
            list_builder.append_null();
        }
    }

    Ok(())
}

/// Appends one null value for a ROS built-in type.
fn append_null_for_builtin(
    ty: &BuiltInType,
    builder: &mut dyn ArrayBuilder,
) -> Result<(), Ros2ReflectionError> {
    match ty {
        BuiltInType::Bool => downcast_builder::<BooleanBuilder>(builder)?.append_null(),
        BuiltInType::Byte | BuiltInType::Char | BuiltInType::UInt8 => {
            downcast_builder::<UInt8Builder>(builder)?.append_null();
        }
        BuiltInType::Int8 => downcast_builder::<Int8Builder>(builder)?.append_null(),
        BuiltInType::Int16 => downcast_builder::<Int16Builder>(builder)?.append_null(),
        BuiltInType::UInt16 => downcast_builder::<UInt16Builder>(builder)?.append_null(),
        BuiltInType::Int32 => downcast_builder::<Int32Builder>(builder)?.append_null(),
        BuiltInType::UInt32 => downcast_builder::<UInt32Builder>(builder)?.append_null(),
        BuiltInType::Int64 => downcast_builder::<Int64Builder>(builder)?.append_null(),
        BuiltInType::UInt64 => downcast_builder::<UInt64Builder>(builder)?.append_null(),
        BuiltInType::Float32 => downcast_builder::<Float32Builder>(builder)?.append_null(),
        BuiltInType::Float64 => downcast_builder::<Float64Builder>(builder)?.append_null(),
        BuiltInType::String(_) | BuiltInType::WString(_) => {
            downcast_builder::<StringBuilder>(builder)?.append_null();
        }
    }

    Ok(())
}
