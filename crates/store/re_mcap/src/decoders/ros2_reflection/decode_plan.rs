//! Decode plans derived from the ROS schemas parsed by `re_ros_msg`.
//!
//! A [`MessageDecodePlan`] closely mirrors an [`re_ros_msg::MessageSchema`], with every type
//! reference resolved to a message ID instead of a type name.
//!
//! This provides decoding a lightweight, immutable plan containing the order and types for
//! traversing the CDR stream without requiring name/type resolution for every message.
//!
//! See [`super::cdr_to_arrow::CdrArrowDecoder`] for how this gets used to map directly from CDR to Arrow.

// TODO(michael): consider making this module a part of `re_ros_msg`, iff it's generic enough.

use anyhow::Context as _;

use re_ros_msg::MessageSchema;

use super::timestamp::TimestampLocation;
use re_ros_msg::message_spec::{
    ArraySize, BuiltInType, ComplexType, MessageSpecification, Type, message_package,
};

/// Immutable, schema-derived instructions for decoding one ROS message type.
///
/// Complex ROS types are resolved to message indexes while this plan is built, so the message
/// decoding hot path neither parses nor looks up schema types.
#[derive(Debug)]
pub(super) struct MessageDecodePlan {
    /// The ROS type name of the root message, e.g. `sensor_msgs/msg/Imu`.
    schema_name: String,

    /// The layout of the root message type, followed by its dependencies in schema order.
    messages: Vec<MessageLayout>,

    /// Where the conventional ROS timestamp sits in the root message, if it has one.
    timestamp_location: TimestampLocation,
}

impl MessageDecodePlan {
    /// The ID of a plan's top-level message type.
    pub(super) const ROOT_ID: usize = 0;

    /// Builds a plan by resolving every message reference in `schema`.
    ///
    /// The returned plan retains no references to `schema`, so it can be cached independently.
    pub(super) fn from_schema(schema: &MessageSchema) -> anyhow::Result<Self> {
        // The schema's own spec goes first, which is what makes the root message `ROOT_ID`.
        let specs = std::iter::chain(std::iter::once(&schema.spec), &schema.dependencies)
            .collect::<Vec<_>>();

        // Assign each schema name a simple numeric ID, in the order that it appears in the spec.
        // We only use those IDs from here on, which keeps name resolution out of the decoding
        // hot path.
        let message_ids = specs
            .iter()
            .enumerate()
            .map(|(id, spec)| (spec.name.as_str(), id))
            .collect::<std::collections::HashMap<_, _>>();

        // Turn each spec into a `MessageLayout`, resolving every field's type to a `ValueLayout`.
        let messages = specs
            .iter()
            .map(|spec| {
                let fields = spec
                    .fields
                    .iter()
                    .map(|field| {
                        Ok(FieldLayout {
                            name: field.name.clone(),
                            value: ValueLayout::from_type(spec, &field.ty, &specs, &message_ids)
                                .with_context(|| {
                                    format!("failed to resolve ROS message field `{}`", field.name)
                                })?,
                        })
                    })
                    .collect::<anyhow::Result<_>>()?;
                Ok(MessageLayout { fields })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self {
            schema_name: schema.spec.name.clone(),
            timestamp_location: TimestampLocation::from_messages(&messages, Self::ROOT_ID),
            messages,
        })
    }

    /// The root ROS message type name represented by this plan.
    pub(super) fn schema_name(&self) -> &str {
        &self.schema_name
    }

    /// Returns the layout of the message with `id`.
    ///
    /// IDs originate from [`Self::ROOT_ID`] or [`ValueLayout::Message`].
    pub(super) fn message(&self, id: usize) -> &MessageLayout {
        &self.messages[id]
    }

    /// The pre-resolved Arrow location of the conventional ROS timestamp fields.
    pub(super) fn timestamp_location(&self) -> &TimestampLocation {
        &self.timestamp_location
    }
}

/// The resolved fields of one ROS message type.
#[derive(Debug)]
pub(super) struct MessageLayout {
    fields: Vec<FieldLayout>,
}

impl MessageLayout {
    /// Fields in ROS wire-order.
    pub(super) fn fields(&self) -> &[FieldLayout] {
        &self.fields
    }
}

/// The name and resolved type of one ROS message field.
#[derive(Debug)]
pub(super) struct FieldLayout {
    name: String,
    value: ValueLayout,
}

impl FieldLayout {
    /// The ROS field name.
    pub(super) fn name(&self) -> &str {
        &self.name
    }

    /// The resolved type of this field.
    pub(super) fn value(&self) -> &ValueLayout {
        &self.value
    }
}

/// The resolved type of a ROS field value.
#[derive(Debug)]
pub(super) enum ValueLayout {
    /// A scalar ROS built-in type.
    BuiltIn(BuiltInType),

    /// A nested ROS message, identified by an ID in [`MessageDecodePlan`].
    Message(usize),

    /// A fixed-size, bounded, or unbounded ROS array.
    Array { element: Box<Self>, size: ArraySize },
}

impl ValueLayout {
    /// Converts a parsed ROS type into a layout, using the schema's precomputed message IDs.
    fn from_type(
        scope: &MessageSpecification,
        ty: &Type,
        specs: &[&MessageSpecification],
        message_ids: &std::collections::HashMap<&str, usize>,
    ) -> anyhow::Result<Self> {
        Ok(match ty {
            Type::BuiltIn(ty) => Self::BuiltIn(ty.clone()),
            Type::Complex(complex_type) => {
                let full_name = match complex_type {
                    ComplexType::Absolute { package, name } => format!("{package}/{name}"),
                    ComplexType::Relative { name } => match message_package(&scope.name) {
                        Some(package) => format!("{package}/{name}"),
                        None => name.clone(),
                    },
                };
                let id = *message_ids.get(full_name.as_str()).ok_or_else(|| match complex_type {
                    ComplexType::Absolute { .. } => {
                        anyhow::anyhow!("could not resolve complex type `{full_name}`")
                    }
                    ComplexType::Relative { name } => anyhow::anyhow!(
                        "relative ROS type `{name}` must resolve within the containing message package as `{full_name}`, but no such message definition was found"
                    ),
                })?;
                let spec = specs[id];
                if let Some(primitive_type) = spec.underlying_type_if_enum_like()? {
                    Self::BuiltIn(primitive_type.clone())
                } else {
                    Self::Message(id)
                }
            }
            Type::Array { ty, size } => Self::Array {
                element: Box::new(Self::from_type(scope, ty, specs, message_ids)?),
                size: size.clone(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[test]
    fn resolves_relative_message_types_once() {
        let schema = MessageSchema::parse(
            "test/msg/Outer",
            r#"
Inner inner

================================================================================
MSG: test/Inner
uint32 value
"#,
        )
        .unwrap();

        let plan = MessageDecodePlan::from_schema(&schema).unwrap();
        let field = &plan.message(MessageDecodePlan::ROOT_ID).fields()[0];

        assert_matches!(field.value(), ValueLayout::Message(1));
    }

    #[test]
    fn resolves_enum_like_messages_to_their_underlying_scalar() {
        let schema = MessageSchema::parse(
            "test/Message",
            r#"
test/Mode mode

================================================================================
MSG: test/Mode
int8 OFF=0
int8 ON=1
"#,
        )
        .unwrap();

        let plan = MessageDecodePlan::from_schema(&schema).unwrap();
        let field = &plan.message(MessageDecodePlan::ROOT_ID).fields()[0];

        assert_matches!(field.value(), ValueLayout::BuiltIn(BuiltInType::Int8));
    }

    #[test]
    fn rejects_unresolved_relative_message_types() {
        let schema = MessageSchema::parse(
            "test/msg/Message",
            r#"
Time timestamp

================================================================================
MSG: other/Time
uint32 sec
uint32 nanosec
"#,
        )
        .unwrap();

        let err = format!("{:#}", MessageDecodePlan::from_schema(&schema).unwrap_err());

        assert!(err.contains("timestamp"));
        assert!(err.contains("test/Time"));
    }
}
