use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, StringArray, StructArray, UInt8Array, UInt32Array};
use re_lenses::{Lens, LensBuilderError};
use re_lenses_core::Selector;
use re_lenses_core::combinators::Error;
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::datatypes::Rgba32;

use crate::importer_mcap::lenses::helpers::get_field_as;

const LOG_ARCHETYPE: &str = "rcl_interfaces.msg.Log";

/// Creates a lens for `rcl_interfaces/msg/Log` messages.
pub fn log() -> Result<Lens, LensBuilderError> {
    Lens::derive("rcl_interfaces.msg.Log:message")
        .to_component(
            TextLog::descriptor_text(),
            Selector::parse(".")?.pipe(ros2_log_text()),
        )
        .to_component(
            TextLog::descriptor_level(),
            Selector::parse(".level")?.pipe(ros2_log_level()),
        )
        .to_component(
            TextLog::descriptor_color(),
            Selector::parse(".level")?.pipe(ros2_log_color()),
        )
        // TODO(#11098): these should be part of the `TextLog` archetype instead.
        .to_component(
            ComponentDescriptor::partial("file").with_archetype(LOG_ARCHETYPE.into()),
            Selector::parse(".file")?,
        )
        .to_component(
            ComponentDescriptor::partial("function").with_archetype(LOG_ARCHETYPE.into()),
            Selector::parse(".function")?,
        )
        .to_component(
            ComponentDescriptor::partial("line").with_archetype(LOG_ARCHETYPE.into()),
            Selector::parse(".line")?,
        )
        .build()
}

/// Formats the log text as `[{name}] {msg}`.
fn ros2_log_text() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let struct_array = source
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "StructArray".to_owned(),
                actual: source.data_type().clone(),
                context: "ros2_log_text input".to_owned(),
            })?;

        let name_array = get_field_as::<StringArray>(struct_array, "name")?;
        let msg_array = get_field_as::<StringArray>(struct_array, "msg")?;

        let result: StringArray = (0..struct_array.len())
            .map(|i| {
                if struct_array.is_null(i) || msg_array.is_null(i) || name_array.is_null(i) {
                    None
                } else {
                    Some(format!("[{}] {}", name_array.value(i), msg_array.value(i)))
                }
            })
            .collect();

        Ok(Some(Arc::new(result) as ArrayRef))
    }
}

/// Maps ROS 2 numeric log levels to Rerun `TextLogLevel` strings.
fn ros2_log_level() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let levels = source
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "UInt8Array".to_owned(),
                actual: source.data_type().clone(),
                context: "ros2_log_level input".to_owned(),
            })?;

        let result: StringArray = levels
            .iter()
            .map(|maybe_level| {
                maybe_level.map(|level| match level {
                    10 => "DEBUG",
                    20 => "INFO",
                    30 => "WARN",
                    40 => "ERROR",
                    50 => "CRITICAL",
                    _ => "UNKNOWN",
                })
            })
            .collect();

        Ok(Some(Arc::new(result) as ArrayRef))
    }
}

/// Maps ROS 2 numeric log levels to a packed RGBA `u32` color.
fn ros2_log_color() -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    move |source: &ArrayRef| {
        let levels = source
            .as_any()
            .downcast_ref::<UInt8Array>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "UInt8Array".to_owned(),
                actual: source.data_type().clone(),
                context: "ros2_log_color input".to_owned(),
            })?;

        let result: UInt32Array = levels
            .iter()
            .map(|maybe_level| {
                maybe_level.map(|level| match level {
                    20 => Rgba32::from_rgb(0, 128, 255).0,
                    30 => Rgba32::from_rgb(255, 165, 0).0,
                    40 => Rgba32::from_rgb(255, 0, 0).0,
                    50 => Rgba32::from_rgb(139, 0, 0).0,
                    _ => Rgba32::from_rgb(128, 128, 128).0,
                })
            })
            .collect();

        Ok(Some(Arc::new(result) as ArrayRef))
    }
}
